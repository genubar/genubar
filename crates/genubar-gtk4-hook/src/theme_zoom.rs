//! Universal Adwaita color-scheme switching and WebKit/GTK zoom dispatchers.

use crate::action_dispatcher::{MAX_ACTIVATE_DEPTH, dispatch_widget_or_app_action};
use crate::exporter::DbusExporter;
use crate::ffi::{
    GDBusConnection, GType, GtkWidget, adw_style_manager_get_default,
    adw_style_manager_set_color_scheme, g_type_check_instance_is_a, webkit_web_view_get_type,
    webkit_web_view_get_zoom_level, webkit_web_view_set_zoom_level,
};
use crate::hook_log;
use crate::state::HookState;
use crate::widget_tree::get_widget_children;
use genubar_protocol::{
    ACTION_THEME_DARK, ACTION_THEME_LIGHT, ACTION_THEME_SYSTEM, ACTION_ZOOM_IN, ACTION_ZOOM_OUT,
    ACTION_ZOOM_RESET,
};
use libc::c_int;
use std::ptr;

pub(crate) unsafe fn try_handle_adwaita_theme(
    connection: *mut GDBusConnection,
    act_str: &str,
) -> bool {
    let (scheme_val, action_spec): (c_int, &str) = match act_str {
        ACTION_THEME_SYSTEM => (
            0,
            "win.interface-style('default')|win.interface-style('system')|settings.style-variant('follow')|app.style-scheme('default')",
        ),
        ACTION_THEME_LIGHT => (
            1,
            "win.interface-style('force-light')|win.interface-style('light')|settings.style-variant('light')|app.style-scheme('light')",
        ),
        ACTION_THEME_DARK => (
            4,
            "win.interface-style('force-dark')|win.interface-style('dark')|settings.style-variant('dark')|app.style-scheme('dark')",
        ),
        _ => return false,
    };

    let _ = unsafe { dispatch_widget_or_app_action(action_spec, ptr::null_mut()) };

    if let Some(mgr) = unsafe { adw_style_manager_get_default() }
        && !mgr.is_null() {
            unsafe {
                let _ = adw_style_manager_set_color_scheme(mgr, scheme_val);
            }
            hook_log!("Applied Libadwaita color scheme={scheme_val}");
        }

    unsafe {
        DbusExporter::notify_daemon_refresh(connection);
    }
    true
}

pub(crate) unsafe fn read_webkit_zoom_percent(window: *mut GtkWidget) -> Option<i32> {
    if window.is_null() {
        return None;
    }
    let wv_type = unsafe { webkit_web_view_get_type() }?;
    if wv_type == 0 {
        return None;
    }
    unsafe { find_webkit_zoom_recursive(window, 0, wv_type) }
}

unsafe fn find_webkit_zoom_recursive(
    widget: *mut GtkWidget,
    depth: usize,
    wv_type: GType,
) -> Option<i32> {
    if widget.is_null() || depth > MAX_ACTIVATE_DEPTH {
        return None;
    }
    let is_wv = unsafe { g_type_check_instance_is_a(widget, wv_type) }
        .map(|r| r != 0)
        .unwrap_or(false);
    if is_wv {
        let level = unsafe { webkit_web_view_get_zoom_level(widget) }.unwrap_or(1.0);
        return Some((level * 100.0).round() as i32);
    }
    for child in unsafe { get_widget_children(widget, depth == 0) } {
        if let Some(pct) = unsafe { find_webkit_zoom_recursive(child, depth + 1, wv_type) } {
            return Some(pct);
        }
    }
    None
}

// Routes universal zoom actions through intercepted `GActionGroup`s (e.g. `view.zoom-in` on
// NautilusFilesView) or falls back to direct `WebKitWebView` scaling (avoiding Yelp's singleton quirk).
pub(crate) unsafe fn try_handle_universal_zoom(
    connection: *mut GDBusConnection,
    act_str: &str,
) -> bool {
    let (spec, delta): (&str, i32) = match act_str {
        ACTION_ZOOM_IN => (
            "page.zoom-in|view.zoom-in|win.zoom-in|editor.zoom-in|app.zoom-in|slot.zoom-in",
            10,
        ),
        ACTION_ZOOM_OUT => (
            "page.zoom-out|view.zoom-out|win.zoom-out|editor.zoom-out|app.zoom-out|slot.zoom-out",
            -10,
        ),
        ACTION_ZOOM_RESET => (
            "page.zoom-one|view.zoom-standard|win.zoom-default|win.zoom-one|editor.zoom-one|app.zoom-default|slot.zoom-standard",
            0,
        ),
        _ => return false,
    };

    let used_action = unsafe { dispatch_widget_or_app_action(spec, ptr::null_mut()) };
    let zoomed = if used_action {
        HookState::adjust_zoom(delta);
        true
    } else {
        let win = HookState::active_window();
        if let Some(wv_type) = unsafe { webkit_web_view_get_type() } {
            if wv_type != 0 && !win.is_null() {
                // `try_zoom_webkit_web_view` updates `HookState::set_zoom_percent` directly from WebKit's zoom level.
                unsafe { try_zoom_webkit_web_view(win, 0, wv_type, delta) }
            } else {
                false
            }
        } else {
            false
        }
    };

    if zoomed {
        unsafe {
            DbusExporter::notify_daemon_refresh(connection);
        }
    }
    true
}

unsafe fn try_zoom_webkit_web_view(
    widget: *mut GtkWidget,
    depth: usize,
    wv_type: GType,
    delta: i32,
) -> bool {
    if widget.is_null() || depth > MAX_ACTIVATE_DEPTH {
        return false;
    }

    let is_wv = unsafe { g_type_check_instance_is_a(widget, wv_type) }
        .map(|r| r != 0)
        .unwrap_or(false);
    if is_wv {
        let new_level = if delta == 0 {
            1.0
        } else {
            let cur = unsafe { webkit_web_view_get_zoom_level(widget) }.unwrap_or(1.0);
            (cur + (delta as f64) / 100.0).clamp(0.3, 4.0)
        };
        unsafe {
            let _ = webkit_web_view_set_zoom_level(widget, new_level);
        }
        HookState::set_zoom_percent((new_level * 100.0).round() as i32);
        hook_log!("  -> Zoomed WebKitWebView ({widget:?}) to level={new_level:.2}");
        return true;
    }

    for child in unsafe { get_widget_children(widget, depth == 0) } {
        if unsafe { try_zoom_webkit_web_view(child, depth + 1, wv_type, delta) } {
            return true;
        }
    }
    false
}
