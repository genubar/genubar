//! Window scanning, debounced rescan timers, and active window protection.

use crate::exporter::DbusExporter;
use crate::ffi::{
    GApplication, GtkWidget, g_action_group_get_type, g_application_get_default, g_object_unref,
    g_signal_connect_data, g_timeout_add_full, g_type_check_instance_is_a,
    gtk_menu_button_get_type, gtk_widget_get_mapped, gtk_widget_get_root, gtk_widget_get_toplevel,
    gtk_widget_get_visible, gtk_window_get_application, gtk_window_get_type, gtk_window_is_active,
    widget_type_name,
};
use crate::hook_log;
use crate::menu_extractor::MenuExtractor;
use crate::state::HookState;
use libc::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

const RESCAN_DELAYS_MS: [u32; 3] = [150, 500, 1200];
const ACTION_RESCAN_DELAY_MS: u32 = 120;
static RESCAN_PENDING: [AtomicBool; 3] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];
static ACTION_RESCAN_PENDING: AtomicBool = AtomicBool::new(false);

pub unsafe fn rescan_active_window(notify_daemon: bool) {
    let window = HookState::active_window();
    let app = HookState::active_app();
    if window.is_null() || app.is_null() {
        return;
    }
    unsafe {
        let _ = scan_and_export_window(window, app, notify_daemon);
    }
}

pub unsafe fn resolve_app_for_window(
    window: *mut GtkWidget,
    mut app: *mut GApplication,
) -> *mut GApplication {
    if app.is_null() && !window.is_null() {
        app = unsafe { gtk_window_get_application(window) }.unwrap_or(ptr::null_mut());
    }
    if app.is_null() {
        app = HookState::active_app();
    }
    if app.is_null() {
        app = unsafe { g_application_get_default() }.unwrap_or(ptr::null_mut());
    }
    app
}

/// Scans `window` for `GMenuModel`s and exports them over D-Bus.
/// Only updates `HookState::set_active_context` when `found_count > 0` (or when no active
/// context is recorded yet) so secondary menu-less dialogs never overwrite the main window.
pub unsafe fn scan_and_export_window(
    window: *mut GtkWidget,
    app_hint: *mut GApplication,
    notify_daemon: bool,
) -> usize {
    if window.is_null() {
        return 0;
    }
    let app = unsafe { resolve_app_for_window(window, app_hint) };
    if app.is_null() {
        return 0;
    }

    let Some(menu_btn_type) = (unsafe { gtk_menu_button_get_type() }) else {
        return 0;
    };

    let (master_menu, found_count, signature) =
        unsafe { MenuExtractor::build_master_menu_for_window(window, app, menu_btn_type) };
    if found_count == 0 || master_menu.is_null() {
        if !master_menu.is_null() {
            unsafe {
                let _ = g_object_unref(master_menu);
            }
        }
        if HookState::active_window().is_null() {
            HookState::set_active_context(window, app);
        }
        hook_log!(
            "Warning: 0 menu models found on window {} ({:?})",
            unsafe { widget_type_name(window) },
            window
        );
        return 0;
    }

    HookState::record_menu_window(window);
    HookState::set_active_context(window, app);
    unsafe {
        connect_action_group_signals(app);
        connect_action_group_signals(window);
    }
    if HookState::check_and_update_export_signature(window, &signature) {
        let conn = unsafe { DbusExporter::resolve_session_connection(app) };
        if !conn.is_null() {
            unsafe {
                DbusExporter::export_window_objects(conn, app, window, master_menu, notify_daemon);
            }
        }
    }
    unsafe {
        let _ = g_object_unref(master_menu);
    }
    found_count
}

unsafe extern "C" fn on_action_group_list_changed(
    _group: *mut c_void,
    _action_name: *const c_char,
    _user_data: *mut c_void,
) {
    HookState::bump_action_state_epoch();
    unsafe {
        trigger_action_rescan();
    }
}

unsafe extern "C" fn on_action_group_state_or_enabled_changed(
    _group: *mut c_void,
    _action_name: *const c_char,
    _value: *mut c_void,
    _user_data: *mut c_void,
) {
    HookState::bump_action_state_epoch();
    unsafe {
        trigger_action_rescan();
    }
}

/// Connects live `GActionGroup` change signals once per instance so dynamic actions and
/// Check/Radio/enabled state changes (e.g. Nautilus `view.*`) refresh the exported menu.
pub unsafe fn connect_action_group_signals(group: *mut c_void) {
    if group.is_null() {
        return;
    }
    let Some(group_type) = (unsafe { g_action_group_get_type() }) else {
        return;
    };
    if group_type == 0 {
        return;
    }
    let is_group = unsafe { g_type_check_instance_is_a(group, group_type) }
        .map(|r| r != 0)
        .unwrap_or(false);
    if !is_group || !unsafe { HookState::mark_action_group_observed(group) } {
        return;
    }

    for sig in [c"action-added", c"action-removed"] {
        unsafe {
            let _ = g_signal_connect_data(
                group,
                sig.as_ptr(),
                on_action_group_list_changed as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
        }
    }
    for sig in [c"action-enabled-changed", c"action-state-changed"] {
        unsafe {
            let _ = g_signal_connect_data(
                group,
                sig.as_ptr(),
                on_action_group_state_or_enabled_changed as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
        }
    }
}

unsafe extern "C" fn action_rescan_cb(_user_data: *mut c_void) -> c_int {
    ACTION_RESCAN_PENDING.store(false, Ordering::SeqCst);
    unsafe {
        rescan_active_window(true);
    }
    0
}

pub unsafe fn trigger_action_rescan() {
    if HookState::active_window().is_null() || HookState::active_app().is_null() {
        return;
    }
    if !ACTION_RESCAN_PENDING.swap(true, Ordering::SeqCst) {
        unsafe {
            let _ = g_timeout_add_full(
                0,
                ACTION_RESCAN_DELAY_MS,
                action_rescan_cb,
                ptr::null_mut(),
                ptr::null(),
            );
        }
    }
}

unsafe extern "C" fn deferred_rescan_cb(user_data: *mut c_void) -> c_int {
    let slot = user_data as usize;
    if let Some(pending) = RESCAN_PENDING.get(slot) {
        pending.store(false, Ordering::SeqCst);
    }
    unsafe {
        rescan_active_window(true);
    }
    0
}

pub unsafe fn trigger_debounced_rescans() {
    for (slot, &delay_ms) in RESCAN_DELAYS_MS.iter().enumerate() {
        if !RESCAN_PENDING[slot].swap(true, Ordering::SeqCst) {
            unsafe {
                let _ = g_timeout_add_full(
                    0,
                    delay_ms,
                    deferred_rescan_cb,
                    slot as *mut c_void,
                    ptr::null(),
                );
            }
        }
    }
}

pub unsafe fn schedule_window_scan(window: *mut GtkWidget, app_hint: *mut GApplication) {
    if window.is_null() {
        return;
    }
    let app = unsafe { resolve_app_for_window(window, app_hint) };
    if app.is_null() {
        return;
    }

    if HookState::active_window().is_null() {
        HookState::set_active_context(window, app);
    }

    unsafe {
        let found = scan_and_export_window(window, app, true);
        if found > 0 || HookState::active_window() == window {
            trigger_debounced_rescans();
        }
    }
}

unsafe extern "C" fn on_window_mapped(window: *mut GtkWidget, app: *mut GApplication) {
    hook_log!("Window mapped: class={} ptr={window:?}", unsafe {
        widget_type_name(window)
    });
    unsafe {
        schedule_window_scan(window, app);
    }
}

unsafe extern "C" fn on_window_destroyed(window: *mut GtkWidget, _app: *mut GApplication) {
    HookState::remove_destroyed_window(window);
    unsafe {
        rescan_active_window(true);
        trigger_debounced_rescans();
    }
}

unsafe extern "C" fn on_window_active_notify(
    window: *mut GtkWidget,
    _pspec: *mut c_void,
    app: *mut GApplication,
) {
    if window.is_null() {
        return;
    }
    let is_act = unsafe { gtk_window_is_active(window) }.unwrap_or(0) != 0;
    if is_act {
        unsafe {
            let resolved_app = resolve_app_for_window(window, app);
            if !resolved_app.is_null() {
                let _ = scan_and_export_window(window, resolved_app, true);
            }
        }
    }
}

pub unsafe fn connect_and_maybe_scan_window(window: *mut GtkWidget, app: *mut GApplication) {
    if window.is_null() {
        return;
    }
    unsafe {
        let resolved_app = resolve_app_for_window(window, app);
        MenuExtractor::maybe_hide_app_window_menubar(window, resolved_app);
        let _ = g_signal_connect_data(
            window,
            c"map".as_ptr(),
            on_window_mapped as *const c_void,
            app,
            ptr::null(),
            0,
        );
        let _ = g_signal_connect_data(
            window,
            c"destroy".as_ptr(),
            on_window_destroyed as *const c_void,
            app,
            ptr::null(),
            0,
        );
        let _ = g_signal_connect_data(
            window,
            c"notify::is-active".as_ptr(),
            on_window_active_notify as *const c_void,
            app,
            ptr::null(),
            0,
        );

        let already_mapped = gtk_widget_get_mapped(window).unwrap_or(0) != 0
            || gtk_widget_get_visible(window).unwrap_or(0) != 0;
        if already_mapped {
            hook_log!(
                "Window already visible on attach: class={} ptr={window:?}",
                widget_type_name(window)
            );
            schedule_window_scan(window, app);
        }
    }
}

pub unsafe fn maybe_rescan_from_menu_button(menu_button: *mut GtkWidget) {
    let mut win = HookState::active_window();
    if win.is_null() {
        let root_cand = unsafe { gtk_widget_get_root(menu_button) }
            .or_else(|| unsafe { gtk_widget_get_toplevel(menu_button) });
        if let Some(root) = root_cand
            && !root.is_null()
        {
            let win_type = unsafe { gtk_window_get_type() }.unwrap_or(0);
            let is_win = win_type != 0
                && unsafe { g_type_check_instance_is_a(root, win_type) }
                    .map(|r| r != 0)
                    .unwrap_or(false);
            if is_win {
                win = root;
                let app = unsafe { resolve_app_for_window(win, ptr::null_mut()) };
                if !app.is_null() {
                    HookState::set_active_context(win, app);
                }
            }
        }
    }

    if !win.is_null() && !HookState::active_app().is_null() {
        unsafe {
            trigger_debounced_rescans();
        }
    }
}
