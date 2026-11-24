//! Thin D-Bus handler for the in-process `org.genubar.Gtk4Bridge` interface.

use crate::action_dispatcher::dispatch_widget_or_app_action;
use crate::action_states::collect_all_action_states;
use crate::debug_dump::dump_window_introspection;
use crate::exporter::DbusExporter;
use crate::ffi::{
    GDBusConnection, GDBusMethodInvocation, GVariant, build_action_states_tuple_variant,
    build_int_tuple_variant, build_string_tuple_variant,
    g_dbus_method_invocation_return_dbus_error, g_dbus_method_invocation_return_value,
};
use crate::hook_log;
use crate::rescan_active_window;
use crate::state::HookState;
use crate::theme_zoom::{
    read_webkit_zoom_percent, try_handle_adwaita_theme, try_handle_universal_zoom,
};
use crate::variant_utils::extract_activate_args;
use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::ptr;

/// Dispatches incoming `org.genubar.Gtk4Bridge` D-Bus method calls on the GTK main loop.
pub unsafe extern "C" fn on_bridge_method_call(
    connection: *mut GDBusConnection,
    _sender: *const c_char,
    _object_path: *const c_char,
    _interface_name: *const c_char,
    method_name: *const c_char,
    parameters: *mut GVariant,
    invocation: *mut GDBusMethodInvocation,
    _user_data: *mut c_void,
) {
    let m_name = unsafe { CStr::from_ptr(method_name).to_str().unwrap_or("") };

    if m_name == "GetZoomPercent" {
        let pct = unsafe { read_webkit_zoom_percent(HookState::active_window()) }
            .inspect(|&p| {
                HookState::set_zoom_percent(p);
            })
            .unwrap_or_else(HookState::zoom_percent);
        let ret_tuple = unsafe { build_int_tuple_variant(pct) };
        unsafe {
            let _ = g_dbus_method_invocation_return_value(invocation, ret_tuple);
        }
        return;
    }

    if m_name == "GetActionStates" {
        let states = unsafe {
            collect_all_action_states(HookState::active_window(), HookState::active_app())
        };
        let ret_tuple = unsafe { build_action_states_tuple_variant(&states) };
        unsafe {
            let _ = g_dbus_method_invocation_return_value(invocation, ret_tuple);
        }
        return;
    }

    if m_name == "Refresh" {
        unsafe {
            HookState::invalidate_export_signature();
            rescan_active_window(false);
            let _ = g_dbus_method_invocation_return_value(invocation, ptr::null_mut());
        }
        return;
    }

    if m_name == "DumpIntrospection" {
        let report = unsafe {
            dump_window_introspection(HookState::active_window(), HookState::active_app())
        };
        let c_rep = CString::new(report).unwrap_or_default();
        let ret_tuple = unsafe { build_string_tuple_variant(c_rep.as_ptr()) };
        unsafe {
            let _ = g_dbus_method_invocation_return_value(invocation, ret_tuple);
        }
        return;
    }

    if m_name == "Activate" {
        unsafe {
            handle_activate_call(connection, parameters);
            let _ = g_dbus_method_invocation_return_value(invocation, ptr::null_mut());
        }
        return;
    }

    let err_msg = CString::new(format!(
        "Unknown method '{m_name}' on org.genubar.Gtk4Bridge"
    ))
    .unwrap_or_default();
    unsafe {
        let _ = g_dbus_method_invocation_return_dbus_error(
            invocation,
            c"org.freedesktop.DBus.Error.UnknownMethod".as_ptr(),
            err_msg.as_ptr(),
        );
    }
}

unsafe fn handle_activate_call(connection: *mut GDBusConnection, parameters: *mut GVariant) {
    let Some(args) = (unsafe { extract_activate_args(parameters) }) else {
        return;
    };
    let Some(act_str) = args.action_name.as_deref() else {
        return;
    };

    hook_log!(
        "Bridge.Activate requested: action='{act_str}', has_param={}",
        !args.target_param.is_null()
    );
    if !unsafe { try_handle_adwaita_theme(connection, act_str) }
        && !unsafe { try_handle_universal_zoom(connection, act_str) }
        && unsafe { dispatch_widget_or_app_action(act_str, args.target_param) }
    {
        // Notify WindowTracker so toggle states (e.g. Show Hidden Files, Sort)
        // and dynamic items (e.g. Star/Unstar) refresh immediately in the top panel.
        unsafe {
            DbusExporter::notify_daemon_refresh(connection);
        }
    }
}
