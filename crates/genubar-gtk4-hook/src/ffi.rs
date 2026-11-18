//! Low-level C FFI type definitions and dynamic symbol (`dlsym`) wrappers for GLib, GIO, GTK4, and Libadwaita.

use libc::{RTLD_DEFAULT, RTLD_NEXT, c_char, c_int, c_uint, c_ulong, c_void, dlsym};
use std::ffi::CStr;

pub type GApplication = c_void;
pub type GDBusConnection = c_void;
pub type GDBusMethodInvocation = c_void;
pub type GDBusNodeInfo = c_void;
pub type GDBusInterfaceInfo = c_void;
pub type GMenuModel = c_void;
pub type GMenu = c_void;
pub type GtkWidget = c_void;
pub type GVariant = c_void;
pub type GVariantBuilder = c_void;
pub type GVariantType = c_void;
pub type GType = usize;

#[repr(C)]
pub struct GDBusInterfaceVTable {
    pub method_call: Option<
        unsafe extern "C" fn(
            *mut GDBusConnection,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            *mut GVariant,
            *mut GDBusMethodInvocation,
            *mut c_void,
        ),
    >,
    pub get_property: *const c_void,
    pub set_property: *const c_void,
    pub padding: [*mut c_void; 8],
}

unsafe impl Sync for GDBusInterfaceVTable {}
unsafe impl Send for GDBusInterfaceVTable {}

#[inline]
unsafe fn load_sym(name: &[u8]) -> *mut c_void {
    unsafe { dlsym(RTLD_DEFAULT, name.as_ptr() as *const c_char) }
}

#[inline]
pub unsafe fn load_next_sym(name: &[u8]) -> *mut c_void {
    unsafe { dlsym(RTLD_NEXT, name.as_ptr() as *const c_char) }
}

pub fn is_gtk4_loaded_in_process() -> bool {
    !unsafe { load_sym(b"gtk_menu_button_get_type\0") }.is_null()
}

/// Returns the GLib class name of a widget (e.g. `"NautilusWindow"`, `"GtkMenuButton"`).
pub unsafe fn widget_type_name(widget: *mut GtkWidget) -> String {
    if widget.is_null() {
        return String::from("null");
    }
    let Some(ptr) = (unsafe { g_type_name_from_instance(widget) }) else {
        return String::from("unknown");
    };
    if ptr.is_null() {
        return String::from("unknown");
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() }
}

macro_rules! define_dlsym_fns {
    ($(
        $(#[$meta:meta])*
        pub fn $fn_name:ident ( $($arg_name:ident : $arg_ty:ty),* ) -> $ret_ty:ty ;
    )*) => {
        $(
            $(#[$meta])*
            #[inline]
            pub unsafe fn $fn_name ( $($arg_name : $arg_ty),* ) -> Option<$ret_ty> {
                let cname = concat!(stringify!($fn_name), "\0");
                let ptr = unsafe { load_sym(cname.as_bytes()) };
                if ptr.is_null() {
                    None
                } else {
                    let func: unsafe extern "C" fn($($arg_ty),*) -> $ret_ty =
                        unsafe { std::mem::transmute(ptr) };
                    Some(unsafe { func($($arg_name),*) })
                }
            }
        )*
    };
}

define_dlsym_fns! {
    // --- GLib / GObject Introspection & Event Loop ---
    pub fn g_type_check_instance_is_a(instance: *mut c_void, iface_type: GType) -> c_int;
    pub fn g_type_name_from_instance(instance: *mut c_void) -> *const c_char;
    pub fn g_object_ref(object: *mut c_void) -> *mut c_void;
    pub fn g_object_unref(object: *mut c_void) -> ();
    pub fn g_object_weak_ref(
        object: *mut c_void,
        notify: unsafe extern "C" fn(*mut c_void, *mut c_void),
        data: *mut c_void
    ) -> ();
    pub fn g_signal_connect_data(
        instance: *mut c_void,
        detailed_signal: *const c_char,
        c_handler: *const c_void,
        data: *mut c_void,
        destroy_data: *const c_void,
        connect_flags: c_int
    ) -> c_ulong;
    pub fn g_timeout_add_full(
        priority: c_int,
        interval: c_uint,
        function: unsafe extern "C" fn(*mut c_void) -> c_int,
        data: *mut c_void,
        notify: *const c_void
    ) -> c_uint;

    // --- GVariant & Memory API ---
    pub fn g_variant_get_child_value(value: *mut GVariant, index: usize) -> *mut GVariant;
    pub fn g_variant_get_boolean(value: *mut GVariant) -> c_int;
    pub fn g_variant_get_string(value: *mut GVariant, length: *mut usize) -> *const c_char;
    pub fn g_variant_n_children(value: *mut GVariant) -> usize;
    pub fn g_variant_get_variant(value: *mut GVariant) -> *mut GVariant;
    pub fn g_variant_classify(value: *mut GVariant) -> c_int;
    pub fn g_variant_ref(value: *mut GVariant) -> *mut GVariant;
    pub fn g_variant_ref_sink(value: *mut GVariant) -> *mut GVariant;
    pub fn g_variant_unref(value: *mut GVariant) -> ();
    pub fn g_variant_print(value: *mut GVariant, type_annotate: c_int) -> *mut c_char;
    pub fn g_variant_type_new(type_string: *const c_char) -> *mut GVariantType;
    pub fn g_variant_type_free(type_: *mut GVariantType) -> ();
    pub fn g_variant_builder_new(type_: *const GVariantType) -> *mut GVariantBuilder;
    pub fn g_variant_builder_end(builder: *mut GVariantBuilder) -> *mut GVariant;
    pub fn g_variant_builder_unref(builder: *mut GVariantBuilder) -> ();
    pub fn g_dbus_node_info_unref(info: *mut GDBusNodeInfo) -> ();
    pub fn g_free(mem: *mut c_void) -> ();
    pub fn g_strfreev(str_array: *mut *mut c_char) -> ();

    // --- GMenu & GActionGroup API ---
    pub fn g_menu_new() -> *mut GMenu;
    pub fn g_menu_append(
        menu: *mut GMenu,
        label: *const c_char,
        detailed_action: *const c_char
    ) -> ();
    pub fn g_menu_append_submenu(
        menu: *mut GMenu,
        label: *const c_char,
        submenu: *mut GMenuModel
    ) -> ();
    pub fn g_menu_append_section(
        menu: *mut GMenu,
        label: *const c_char,
        section: *mut GMenuModel
    ) -> ();
    pub fn g_menu_model_get_n_items(model: *mut GMenuModel) -> c_int;
    pub fn g_action_group_get_type() -> GType;
    pub fn g_action_group_list_actions(action_group: *mut c_void) -> *mut *mut c_char;
    pub fn g_action_group_has_action(action_group: *mut c_void, action_name: *const c_char) -> c_int;
    pub fn g_action_group_get_action_enabled(
        action_group: *mut c_void,
        action_name: *const c_char
    ) -> c_int;
    pub fn g_action_group_get_action_state(
        action_group: *mut c_void,
        action_name: *const c_char
    ) -> *mut GVariant;
    pub fn g_action_group_activate_action(
        action_group: *mut c_void,
        action_name: *const c_char,
        parameter: *mut GVariant
    ) -> ();
    pub fn g_action_parse_detailed_name(
        detailed_name: *const c_char,
        action_name: *mut *mut c_char,
        target_value: *mut *mut GVariant,
        error: *mut *mut c_void
    ) -> c_int;

    // --- GDBus & GApplication API ---
    pub fn g_application_get_dbus_connection(app: *mut GApplication) -> *mut GDBusConnection;
    pub fn g_application_get_default() -> *mut GApplication;
    pub fn g_bus_get_sync(
        bus_type: c_int,
        cancellable: *mut c_void,
        error: *mut *mut c_void
    ) -> *mut GDBusConnection;
    pub fn g_dbus_connection_get_unique_name(conn: *mut GDBusConnection) -> *const c_char;
    pub fn g_dbus_connection_export_menu_model(
        conn: *mut GDBusConnection,
        object_path: *const c_char,
        menu: *mut GMenuModel,
        error: *mut *mut c_void
    ) -> u32;
    pub fn g_dbus_connection_unexport_menu_model(
        conn: *mut GDBusConnection,
        export_id: u32
    ) -> ();
    pub fn g_dbus_connection_export_action_group(
        conn: *mut GDBusConnection,
        object_path: *const c_char,
        action_group: *mut c_void,
        error: *mut *mut c_void
    ) -> u32;
    pub fn g_dbus_connection_unexport_action_group(
        conn: *mut GDBusConnection,
        export_id: u32
    ) -> ();
    pub fn g_dbus_node_info_new_for_xml(
        xml_data: *const c_char,
        error: *mut *mut c_void
    ) -> *mut GDBusNodeInfo;
    pub fn g_dbus_node_info_lookup_interface(
        info: *mut GDBusNodeInfo,
        name: *const c_char
    ) -> *mut GDBusInterfaceInfo;
    pub fn g_dbus_connection_register_object(
        conn: *mut GDBusConnection,
        object_path: *const c_char,
        interface_info: *mut GDBusInterfaceInfo,
        vtable: *const GDBusInterfaceVTable,
        user_data: *mut c_void,
        destroy_notify: *const c_void,
        error: *mut *mut c_void
    ) -> u32;
    pub fn g_dbus_method_invocation_return_value(
        invocation: *mut GDBusMethodInvocation,
        parameters: *mut GVariant
    ) -> ();
    pub fn g_dbus_method_invocation_return_dbus_error(
        invocation: *mut GDBusMethodInvocation,
        error_name: *const c_char,
        error_message: *const c_char
    ) -> ();
    pub fn g_dbus_connection_call(
        conn: *mut GDBusConnection,
        bus_name: *const c_char,
        object_path: *const c_char,
        interface_name: *const c_char,
        method_name: *const c_char,
        parameters: *mut GVariant,
        reply_type: *const c_void,
        flags: c_int,
        timeout_msec: c_int,
        cancellable: *mut c_void,
        callback: *const c_void,
        user_data: *mut c_void
    ) -> ();

    // --- GTK4 & GTK3 Widget, Container, MenuButton & PopoverMenu API ---
    pub fn gtk_window_get_type() -> GType;
    pub fn gtk_application_window_get_type() -> GType;
    pub fn gtk_application_window_set_show_menubar(
        window: *mut GtkWidget,
        show_menubar: c_int
    ) -> ();
    pub fn gtk_application_get_menubar(application: *mut GApplication) -> *mut GMenuModel;
    pub fn gtk_container_get_type() -> GType;
    pub fn gtk_menu_button_get_type() -> GType;
    pub fn gtk_popover_menu_get_type() -> GType;
    pub fn gtk_widget_get_root(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_toplevel(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_window_get_titlebar(window: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_container_forall(
        container: *mut GtkWidget,
        callback: unsafe extern "C" fn(*mut GtkWidget, *mut c_void),
        callback_data: *mut c_void
    ) -> ();
    pub fn gtk_widget_get_first_child(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_next_sibling(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_parent(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_visible(widget: *mut GtkWidget) -> c_int;
    pub fn gtk_widget_set_visible(widget: *mut GtkWidget, visible: c_int) -> ();
    pub fn gtk_widget_get_tooltip_text(widget: *mut GtkWidget) -> *const c_char;
    pub fn gtk_button_get_label(button: *mut GtkWidget) -> *const c_char;
    pub fn gtk_widget_activate_action_variant(
        widget: *mut GtkWidget,
        name: *const c_char,
        args: *mut GVariant
    ) -> c_int;
    pub fn gtk_window_get_focus(window: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_window_is_active(window: *mut GtkWidget) -> c_int;
    pub fn gtk_widget_get_mapped(widget: *mut GtkWidget) -> c_int;
    pub fn gtk_window_get_application(window: *mut GtkWidget) -> *mut GApplication;
    pub fn gtk_application_get_active_window(application: *mut GApplication) -> *mut GtkWidget;
    pub fn gtk_stack_get_type() -> GType;
    pub fn gtk_stack_get_visible_child(stack: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_menu_button_get_menu_model(menu_button: *mut GtkWidget) -> *mut GMenuModel;
    pub fn gtk_menu_button_get_popover(menu_button: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_popover_menu_get_menu_model(popover: *mut GtkWidget) -> *mut GMenuModel;
    pub fn gtk_menu_button_get_icon_name(menu_button: *mut GtkWidget) -> *const c_char;
    pub fn gtk_menu_button_get_label(menu_button: *mut GtkWidget) -> *const c_char;

    // --- Libadwaita StyleManager & ViewStack API ---
    pub fn adw_style_manager_get_default() -> *mut c_void;
    pub fn adw_style_manager_get_color_scheme(manager: *mut c_void) -> c_int;
    pub fn adw_style_manager_set_color_scheme(manager: *mut c_void, color_scheme: c_int) -> ();
    pub fn adw_view_stack_get_type() -> GType;
    pub fn adw_view_stack_get_visible_child(stack: *mut GtkWidget) -> *mut GtkWidget;

    // --- WebKitGTK 6.0 Zoom API (for Yelp / Epiphany / Devhelp) ---
    pub fn webkit_web_view_get_type() -> GType;
    pub fn webkit_web_view_get_zoom_level(web_view: *mut GtkWidget) -> f64;
    pub fn webkit_web_view_set_zoom_level(web_view: *mut GtkWidget, zoom_level: f64) -> ();
}

pub unsafe fn build_int_tuple_variant(val: i32) -> *mut GVariant {
    type FnVariantNewInt = unsafe extern "C" fn(*const c_char, i32) -> *mut GVariant;
    let ptr = unsafe { load_sym(b"g_variant_new\0") };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let func: FnVariantNewInt = unsafe { std::mem::transmute(ptr) };
    unsafe { func(c"(i)".as_ptr(), val) }
}

pub unsafe fn build_string_tuple_variant(val: *const c_char) -> *mut GVariant {
    type FnVariantNewStr = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut GVariant;
    let ptr = unsafe { load_sym(b"g_variant_new\0") };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let func: FnVariantNewStr = unsafe { std::mem::transmute(ptr) };
    unsafe { func(c"(s)".as_ptr(), val) }
}

pub unsafe fn build_register_window_variant(
    pid: u32,
    bus_name: *const c_char,
    menu_path: *const c_char,
    app_path: *const c_char,
    win_path: *const c_char,
) -> *mut GVariant {
    type FnVariantNewReg = unsafe extern "C" fn(
        *const c_char,
        u32,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> *mut GVariant;

    let ptr = unsafe { load_sym(b"g_variant_new\0") };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let func: FnVariantNewReg = unsafe { std::mem::transmute(ptr) };
    unsafe { func(c"(ussss)".as_ptr(), pid, bus_name, menu_path, app_path, win_path) }
}

/// Serializes `(key, enabled, kind, value)` entries into a GLib `(a(sbss))` tuple `GVariant`.
pub unsafe fn build_action_states_tuple_variant(
    entries: &[(String, bool, &'static str, String)],
) -> *mut GVariant {
    let Some(arr_type) = (unsafe { g_variant_type_new(c"a(sbss)".as_ptr()) }) else {
        return std::ptr::null_mut();
    };
    if arr_type.is_null() {
        return std::ptr::null_mut();
    }
    let builder_opt = unsafe { g_variant_builder_new(arr_type) };
    unsafe {
        let _ = g_variant_type_free(arr_type);
    }
    let Some(builder) = builder_opt else {
        return std::ptr::null_mut();
    };
    if builder.is_null() {
        return std::ptr::null_mut();
    }

    let add_ptr = unsafe { load_sym(b"g_variant_builder_add\0") };
    let new_ptr = unsafe { load_sym(b"g_variant_new\0") };
    if add_ptr.is_null() || new_ptr.is_null() {
        unsafe {
            let _ = g_variant_builder_unref(builder);
        }
        return std::ptr::null_mut();
    }

    type FnBuilderAddEntry = unsafe extern "C" fn(
        *mut GVariantBuilder,
        *const c_char,
        *const c_char,
        c_int,
        *const c_char,
        *const c_char,
    );
    type FnVariantWrapArray = unsafe extern "C" fn(*const c_char, *mut GVariant) -> *mut GVariant;

    let add_fn: FnBuilderAddEntry = unsafe { std::mem::transmute(add_ptr) };
    let new_fn: FnVariantWrapArray = unsafe { std::mem::transmute(new_ptr) };

    for (key, enabled, kind, value) in entries {
        let c_key = std::ffi::CString::new(key.as_str()).unwrap_or_default();
        let c_kind = std::ffi::CString::new(*kind).unwrap_or_default();
        let c_val = std::ffi::CString::new(value.as_str()).unwrap_or_default();
        unsafe {
            add_fn(
                builder,
                c"(sbss)".as_ptr(),
                c_key.as_ptr(),
                c_int::from(*enabled),
                c_kind.as_ptr(),
                c_val.as_ptr(),
            );
        }
    }

    let arr_variant = unsafe { g_variant_builder_end(builder) }.unwrap_or(std::ptr::null_mut());
    unsafe {
        let _ = g_variant_builder_unref(builder);
    }
    if arr_variant.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { new_fn(c"(@a(sbss))".as_ptr(), arr_variant) }
}