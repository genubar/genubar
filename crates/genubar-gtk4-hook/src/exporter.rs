//! Handles GDBus object export/unexport for menus, action groups, and the custom `Gtk4Bridge` interface.

use crate::bridge::on_bridge_method_call;
use crate::ffi::{
    GApplication, GDBusConnection, GDBusInterfaceVTable, GMenu, GMenuModel, GtkWidget,
    build_register_window_variant, g_application_get_dbus_connection, g_bus_get_sync,
    g_dbus_connection_call, g_dbus_connection_export_action_group,
    g_dbus_connection_export_menu_model, g_dbus_connection_get_unique_name,
    g_dbus_connection_register_object, g_dbus_connection_unexport_action_group,
    g_dbus_connection_unexport_menu_model, g_dbus_node_info_lookup_interface,
    g_dbus_node_info_new_for_xml, g_dbus_node_info_unref,
};
use crate::state::HookState;
use genubar_protocol::{
    GTK4_APP_PATH_C, GTK4_BRIDGE_INTERFACE_C, GTK4_BRIDGE_PATH_C, GTK4_MENU_PATH_C,
    GTK4_WIN_PATH_C, TRACKER_BUS_NAME_C, TRACKER_INTERFACE_C, TRACKER_OBJECT_PATH_C,
};
use libc::getpid;
use std::ptr;

const BRIDGE_INTROSPECTION_XML: &std::ffi::CStr = c"<node>\
  <interface name='org.genubar.Gtk4Bridge'>\
    <method name='Activate'>\
      <arg type='s' name='action' direction='in'/>\
      <arg type='av' name='param' direction='in'/>\
    </method>\
    <method name='GetZoomPercent'>\
      <arg type='i' name='pct' direction='out'/>\
    </method>\
    <method name='GetActionStates'>\
      <arg type='a(sbss)' name='states' direction='out'/>\
    </method>\
    <method name='Refresh'/>\
    <method name='DumpIntrospection'>\
      <arg type='s' name='report' direction='out'/>\
    </method>\
  </interface>\
</node>";

pub struct DbusExporter;

impl DbusExporter {
    /// Resolves the session `GDBusConnection` from `GApplication` or falls back to `g_bus_get_sync`.
    pub unsafe fn resolve_session_connection(app: *mut GApplication) -> *mut GDBusConnection {
        let mut conn = unsafe { g_application_get_dbus_connection(app) }.unwrap_or(ptr::null_mut());
        if conn.is_null() {
            conn = unsafe { g_bus_get_sync(2, ptr::null_mut(), ptr::null_mut()) }
                .unwrap_or(ptr::null_mut());
        }
        conn
    }

    // Unexport prior D-Bus object IDs before exporting on the same path, because GIO
    // fails with `G_IO_ERROR_EXISTS` (returning 0) on rescan otherwise.
    pub unsafe fn export_window_objects(
        conn: *mut GDBusConnection,
        app: *mut GApplication,
        window: *mut GtkWidget,
        master_menu: *mut GMenu,
        notify_daemon: bool,
    ) {
        if conn.is_null() {
            return;
        }

        // 1. Unexport previous GMenuModel first, then export the updated master_menu
        let prev_menu = HookState::swap_menu_export_id(0);
        if prev_menu != 0 {
            unsafe {
                let _ = g_dbus_connection_unexport_menu_model(conn, prev_menu);
            }
        }
        if let Some(new_menu_id) = unsafe {
            g_dbus_connection_export_menu_model(
                conn,
                GTK4_MENU_PATH_C.as_ptr(),
                master_menu as *mut GMenuModel,
                ptr::null_mut(),
            )
        } {
            let _ = HookState::swap_menu_export_id(new_menu_id);
        }

        // 2. Unexport previous GActionGroups first, then export current app and window groups
        let prev_app = HookState::swap_app_action_export_id(0);
        if prev_app != 0 {
            unsafe {
                let _ = g_dbus_connection_unexport_action_group(conn, prev_app);
            }
        }
        if let Some(new_app_id) = unsafe {
            g_dbus_connection_export_action_group(
                conn,
                GTK4_APP_PATH_C.as_ptr(),
                app,
                ptr::null_mut(),
            )
        } {
            let _ = HookState::swap_app_action_export_id(new_app_id);
        }

        let prev_win = HookState::swap_win_action_export_id(0);
        if prev_win != 0 {
            unsafe {
                let _ = g_dbus_connection_unexport_action_group(conn, prev_win);
            }
        }
        if let Some(new_win_id) = unsafe {
            g_dbus_connection_export_action_group(
                conn,
                GTK4_WIN_PATH_C.as_ptr(),
                window,
                ptr::null_mut(),
            )
        } {
            let _ = HookState::swap_win_action_export_id(new_win_id);
        }

        // 3. Register the `org.genubar.Gtk4Bridge` object once per process
        if HookState::bridge_export_id() == 0 {
            unsafe {
                Self::register_bridge_interface(conn);
            }
        }

        // 4. Notify the Genubar daemon (`org.genubar.WindowTracker`) only when requested
        if notify_daemon {
            unsafe {
                Self::notify_daemon_refresh(conn);
            }
        }
    }

    unsafe fn register_bridge_interface(conn: *mut GDBusConnection) {
        let Some(node_info) = (unsafe {
            g_dbus_node_info_new_for_xml(BRIDGE_INTROSPECTION_XML.as_ptr(), ptr::null_mut())
        }) else {
            return;
        };
        if node_info.is_null() {
            return;
        }

        let Some(iface_info) = (unsafe {
            g_dbus_node_info_lookup_interface(node_info, GTK4_BRIDGE_INTERFACE_C.as_ptr())
        }) else {
            unsafe {
                let _ = g_dbus_node_info_unref(node_info);
            }
            return;
        };
        if iface_info.is_null() {
            unsafe {
                let _ = g_dbus_node_info_unref(node_info);
            }
            return;
        }

        static VTABLE: GDBusInterfaceVTable = GDBusInterfaceVTable {
            method_call: Some(on_bridge_method_call),
            get_property: ptr::null(),
            set_property: ptr::null(),
            padding: [ptr::null_mut(); 8],
        };

        if let Some(reg_id) = unsafe {
            g_dbus_connection_register_object(
                conn,
                GTK4_BRIDGE_PATH_C.as_ptr(),
                iface_info,
                &VTABLE,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
            )
        } {
            HookState::set_bridge_export_id(reg_id);
        }
        unsafe {
            let _ = g_dbus_node_info_unref(node_info);
        }
    }

    /// Sends an asynchronous `RegisterGtk4Window` D-Bus call to `org.genubar.WindowTracker`.
    pub unsafe fn notify_daemon_refresh(conn: *mut GDBusConnection) {
        if conn.is_null() {
            return;
        }
        let Some(bus_name_ptr) = (unsafe { g_dbus_connection_get_unique_name(conn) }) else {
            return;
        };
        if bus_name_ptr.is_null() {
            return;
        }

        let pid = unsafe { getpid() as u32 };
        let payload = unsafe {
            build_register_window_variant(
                pid,
                bus_name_ptr,
                GTK4_MENU_PATH_C.as_ptr(),
                GTK4_APP_PATH_C.as_ptr(),
                GTK4_WIN_PATH_C.as_ptr(),
            )
        };
        if payload.is_null() {
            return;
        }

        unsafe {
            let _ = g_dbus_connection_call(
                conn,
                TRACKER_BUS_NAME_C.as_ptr(),
                TRACKER_OBJECT_PATH_C.as_ptr(),
                TRACKER_INTERFACE_C.as_ptr(),
                c"RegisterGtk4Window".as_ptr(),
                payload,
                ptr::null(),
                1,
                -1,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
            );
        }
    }
}
