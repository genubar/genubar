//! Structured live `GActionGroup` state collector for `org.genubar.Gtk4Bridge.GetActionStates`.

use crate::ffi::{
    GApplication, GtkWidget, adw_style_manager_get_color_scheme, adw_style_manager_get_default,
    g_action_group_get_action_enabled, g_action_group_get_action_state,
    g_action_group_list_actions, g_free, g_strfreev, g_variant_classify, g_variant_get_boolean,
    g_variant_get_string, g_variant_print, g_variant_unref,
};
use crate::state::HookState;
use crate::widget_tree::collect_live_widgets;
use libc::{c_int, c_void};
use std::collections::HashSet;
use std::ffi::CStr;
use std::ptr;

const G_VARIANT_CLASS_BOOLEAN: c_int = b'b' as c_int;
const G_VARIANT_CLASS_STRING: c_int = b's' as c_int;

pub type ActionStateRecord = (String, bool, &'static str, String);

/// Collects all live `GAction` states (`app`, `win`, widget groups, and Adwaita scheme)
/// as structured `(full_key, enabled, kind, value)` records.
pub unsafe fn collect_all_action_states(
    window: *mut GtkWidget,
    app: *mut GApplication,
) -> Vec<ActionStateRecord> {
    let mut out = Vec::new();
    if window.is_null() {
        return out;
    }

    let mut live_widgets = HashSet::new();
    unsafe {
        collect_live_widgets(window, 0, &mut live_widgets);
    }

    if !app.is_null() {
        unsafe {
            append_group_states("app", app, &mut out);
        }
    }
    unsafe {
        append_group_states("win", window, &mut out);
    }

    for (_w_addr, prefix, g_addr) in HookState::get_live_action_groups(&live_widgets) {
        unsafe {
            append_group_states(&prefix, g_addr as *mut c_void, &mut out);
        }
    }

    for (act_name, enabled) in HookState::get_live_class_action_states(&live_widgets) {
        if !out.iter().any(|(existing_key, ..)| existing_key == &act_name) {
            out.push((act_name, enabled, "none", String::new()));
        }
    }

    if let Some(mgr) = unsafe { adw_style_manager_get_default() }
        && !mgr.is_null() {
            let scheme = unsafe { adw_style_manager_get_color_scheme(mgr) }.unwrap_or(0);
            let scheme_str = match scheme {
                1 => "light",
                4 => "dark",
                _ => "system",
            };
            out.push((
                String::from("genubar.theme"),
                true,
                "str",
                String::from(scheme_str),
            ));
        }

    out
}

unsafe fn append_group_states(
    prefix: &str,
    group_ptr: *mut c_void,
    out: &mut Vec<ActionStateRecord>,
) {
    let Some(list_ptr) = (unsafe { g_action_group_list_actions(group_ptr) }) else {
        return;
    };
    if list_ptr.is_null() {
        return;
    }

    let mut idx = 0usize;
    loop {
        let item_ptr = unsafe { *list_ptr.add(idx) };
        if item_ptr.is_null() {
            break;
        }
        if let Ok(name) = unsafe { CStr::from_ptr(item_ptr).to_str() } {
            let enabled = unsafe { g_action_group_get_action_enabled(group_ptr, item_ptr) }
                .map(|r| r != 0)
                .unwrap_or(true);

            let state_ptr = unsafe { g_action_group_get_action_state(group_ptr, item_ptr) }
                .unwrap_or(ptr::null_mut());

            let (kind, val_str) = if !state_ptr.is_null() {
                let class_code = unsafe { g_variant_classify(state_ptr) }.unwrap_or(0);
                let parsed = if class_code == G_VARIANT_CLASS_BOOLEAN {
                    let b = unsafe { g_variant_get_boolean(state_ptr) }.unwrap_or(0) != 0;
                    ("bool", if b { String::from("true") } else { String::from("false") })
                } else if class_code == G_VARIANT_CLASS_STRING {
                    let s_ptr = unsafe { g_variant_get_string(state_ptr, ptr::null_mut()) }
                        .unwrap_or(ptr::null());
                    let s = if s_ptr.is_null() {
                        String::new()
                    } else {
                        unsafe { CStr::from_ptr(s_ptr).to_string_lossy().to_string() }
                    };
                    ("str", s)
                } else {
                    let printed = unsafe { g_variant_print(state_ptr, 0) }.unwrap_or(ptr::null_mut());
                    let txt = if !printed.is_null() {
                        let s = unsafe { CStr::from_ptr(printed).to_string_lossy().to_string() };
                        unsafe {
                            let _ = g_free(printed as *mut c_void);
                        }
                        s
                    } else {
                        String::new()
                    };
                    ("other", txt)
                };
                unsafe {
                    let _ = g_variant_unref(state_ptr);
                }
                parsed
            } else {
                ("none", String::new())
            };

            out.push((format!("{prefix}.{name}"), enabled, kind, val_str));
        }
        idx += 1;
    }

    unsafe {
        let _ = g_strfreev(list_ptr);
    }
}
