//! Exported C `LD_PRELOAD` symbol interceptors for GTK4, GTK3, GIO, and GModule.

use crate::ffi::{
    GApplication, GMenuModel, GtkWidget, g_signal_connect_data, is_gtk4_loaded_in_process,
    widget_type_name,
};
use crate::gi_hook::{
    REAL_G_APPLICATION_RUN, REAL_GTK_APP_ADD_WINDOW, REAL_GTK_MENU_BTN_SET_MODEL,
    REAL_GTK_MENU_BTN_SET_POPOVER, REAL_GTK_POPOVER_BIND_MODEL, REAL_GTK_POPOVER_NEW_FROM_MODEL,
    REAL_GTK_WIDGET_ACTION_SET_ENABLED, REAL_GTK_WIDGET_INSERT_ACT_GRP, REAL_GTK_WINDOW_PRESENT,
    REAL_GTK_WINDOW_SET_APP, ensure_gtk_symbols_global, intercept_g_module_symbol,
    resolve_real_sym,
};
use crate::hook_log;
use crate::instance_guard::terminate_stale_unhooked_instances;
use crate::scanner::{
    connect_action_group_signals, connect_and_maybe_scan_window, maybe_rescan_from_menu_button,
    schedule_window_scan, trigger_action_rescan,
};
use crate::state::HookState;
use libc::{c_char, c_int, c_void};
use std::ffi::CStr;
use std::ptr;

macro_rules! define_gtk_hook {
    // Void-returning wrapper: calls real GTK/GLib symbol if resolved, then runs `$body`.
    (
        $(#[$meta:meta])*
        pub unsafe extern "C" fn $fn_name:ident ( $($arg:ident : $arg_ty:ty),* $(,)? )
        using ($c_sym:expr, $cache_slot:expr)
        $body:block
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $fn_name ( $($arg : $arg_ty),* ) {
            type RealFn = unsafe extern "C" fn($($arg_ty),*);
            unsafe { ensure_gtk_symbols_global() };
            let real_fn_ptr = unsafe { resolve_real_sym($c_sym, &$cache_slot) };
            if !real_fn_ptr.is_null() {
                let real_fn: RealFn = unsafe { std::mem::transmute(real_fn_ptr) };
                unsafe { real_fn($($arg),*) };
            }
            $body
        }
    };

    // Value-returning wrapper: resolves real symbol, returns `$default` on failure, binds result to `$ret`, then runs `$body`.
    (
        $(#[$meta:meta])*
        pub unsafe extern "C" fn $fn_name:ident ( $($arg:ident : $arg_ty:ty),* $(,)? ) -> $ret_ty:ty
        => using ($c_sym:expr, $cache_slot:expr, fallback = $default:expr)
        |$ret:ident| $body:block
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $fn_name ( $($arg : $arg_ty),* ) -> $ret_ty {
            type RealFn = unsafe extern "C" fn($($arg_ty),*) -> $ret_ty;
            unsafe { ensure_gtk_symbols_global() };
            let real_fn_ptr = unsafe { resolve_real_sym($c_sym, &$cache_slot) };
            if real_fn_ptr.is_null() {
                return $default;
            }
            let real_fn: RealFn = unsafe { std::mem::transmute(real_fn_ptr) };
            let $ret = unsafe { real_fn($($arg),*) };
            $body
            $ret
        }
    };
}

unsafe extern "C" fn on_window_added(
    app: *mut GApplication,
    window: *mut GtkWidget,
    _user_data: *mut c_void,
) {
    unsafe {
        connect_and_maybe_scan_window(window, app);
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_window_present(window: *mut GtkWidget)
    using (b"gtk_window_present\0", REAL_GTK_WINDOW_PRESENT)
    {
        if !window.is_null() {
            hook_log!(
                "gtk_window_present intercepted: class={} ptr={window:?}",
                unsafe { widget_type_name(window) }
            );
            unsafe {
                schedule_window_scan(window, ptr::null_mut());
            }
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_window_set_application(
        window: *mut GtkWidget,
        app: *mut GApplication,
    )
    using (b"gtk_window_set_application\0", REAL_GTK_WINDOW_SET_APP)
    {
        if !window.is_null() && !app.is_null() {
            unsafe {
                connect_and_maybe_scan_window(window, app);
            }
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_application_add_window(
        app: *mut GApplication,
        window: *mut GtkWidget,
    )
    using (b"gtk_application_add_window\0", REAL_GTK_APP_ADD_WINDOW)
    {
        if !window.is_null() && !app.is_null() {
            unsafe {
                connect_and_maybe_scan_window(window, app);
            }
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_menu_button_set_menu_model(
        menu_button: *mut GtkWidget,
        menu_model: *mut GMenuModel,
    )
    using (b"gtk_menu_button_set_menu_model\0", REAL_GTK_MENU_BTN_SET_MODEL)
    {
        if !menu_button.is_null() && !menu_model.is_null() {
            unsafe {
                maybe_rescan_from_menu_button(menu_button);
            }
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_menu_button_set_popover(
        menu_button: *mut GtkWidget,
        popover: *mut GtkWidget,
    )
    using (b"gtk_menu_button_set_popover\0", REAL_GTK_MENU_BTN_SET_POPOVER)
    {
        if !menu_button.is_null() && !popover.is_null() {
            unsafe {
                maybe_rescan_from_menu_button(menu_button);
            }
        }
    }
}

define_gtk_hook! {
    /// Captures `(popover -> GMenuModel)` bindings created in GTK3 apps (`gnome-disks`, `gnome-boxes`).
    pub unsafe extern "C" fn gtk_popover_new_from_model(
        relative_to: *mut GtkWidget,
        model: *mut GMenuModel,
    ) -> *mut GtkWidget
    => using (b"gtk_popover_new_from_model\0", REAL_GTK_POPOVER_NEW_FROM_MODEL, fallback = ptr::null_mut())
    |popover| {
        if !popover.is_null() && !model.is_null() {
            HookState::record_popover_menu_model(popover, model);
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_popover_bind_model(
        popover: *mut GtkWidget,
        model: *mut GMenuModel,
        action_namespace: *const c_char,
    )
    using (b"gtk_popover_bind_model\0", REAL_GTK_POPOVER_BIND_MODEL)
    {
        if !popover.is_null() {
            HookState::record_popover_menu_model(popover, model);
        }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_widget_insert_action_group(
        widget: *mut GtkWidget,
        name: *const c_char,
        group: *mut c_void,
    )
    using (b"gtk_widget_insert_action_group\0", REAL_GTK_WIDGET_INSERT_ACT_GRP)
    {
        if !widget.is_null() && !name.is_null()
            && let Ok(prefix) = unsafe { CStr::from_ptr(name).to_str() }
                && HookState::record_widget_action_group(widget, prefix, group) {
                    if !group.is_null() {
                        unsafe {
                            connect_action_group_signals(group);
                        }
                    }
                    unsafe {
                        trigger_action_rescan();
                    }
                }
    }
}

define_gtk_hook! {
    pub unsafe extern "C" fn gtk_widget_action_set_enabled(
        widget: *mut GtkWidget,
        action_name: *const c_char,
        enabled: c_int,
    )
    using (b"gtk_widget_action_set_enabled\0", REAL_GTK_WIDGET_ACTION_SET_ENABLED)
    {
        if !widget.is_null() && !action_name.is_null()
            && let Ok(action_str) = unsafe { CStr::from_ptr(action_name).to_str() }
                && HookState::record_widget_class_action_enabled(widget, action_str, enabled != 0) {
                    unsafe {
                        trigger_action_rescan();
                    }
                }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn g_module_symbol(
    module: *mut c_void,
    symbol_name: *const c_char,
    symbol: *mut *mut c_void,
) -> c_int {
    unsafe { intercept_g_module_symbol(module, symbol_name, symbol) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn g_application_run(
    app: *mut GApplication,
    argc: c_int,
    argv: *mut *mut c_char,
) -> c_int {
    type FnGApplicationRun =
        unsafe extern "C" fn(*mut GApplication, c_int, *mut *mut c_char) -> c_int;

    unsafe { ensure_gtk_symbols_global() };
    let real_fn_ptr = unsafe { resolve_real_sym(b"g_application_run\0", &REAL_G_APPLICATION_RUN) };
    if real_fn_ptr.is_null() {
        return -1;
    }
    let real_g_application_run: FnGApplicationRun = unsafe { std::mem::transmute(real_fn_ptr) };

    let gtk_present = is_gtk4_loaded_in_process();
    hook_log!("g_application_run intercepted (gtk_loaded={gtk_present})");

    let is_genubar_proc = std::fs::read_to_string("/proc/self/comm")
        .map(|s| s.trim().starts_with("genubar"))
        .unwrap_or(false);

    if is_genubar_proc {
        unsafe {
            libc::unsetenv(c"LD_PRELOAD".as_ptr());
            return real_g_application_run(app, argc, argv);
        }
    }

    if gtk_present && !app.is_null() {
        HookState::set_active_context(HookState::active_window(), app);
        terminate_stale_unhooked_instances();
    }

    if !app.is_null() {
        unsafe {
            let _ = g_signal_connect_data(
                app,
                c"window-added".as_ptr(),
                on_window_added as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
        }
    }

    unsafe {
        libc::unsetenv(c"LD_PRELOAD".as_ptr());
        real_g_application_run(app, argc, argv)
    }
}
