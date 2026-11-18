//! In-process GTK4 action dispatch across widget groups, subtree muxer, and app/window roots.

use crate::button_heuristics::is_ignored_widget_class;
use crate::ffi::{
    GVariant, GtkWidget, g_action_group_activate_action, g_action_group_get_action_enabled,
    g_action_group_has_action, g_variant_ref, gtk_widget_activate_action_variant,
    gtk_window_get_focus, widget_type_name,
};
use crate::hook_log;
use crate::state::HookState;
use crate::variant_utils::DetailedActionName;
use crate::widget_tree::{collect_live_widgets, get_widget_children};
use libc::{c_char, c_void};
use std::collections::HashSet;
use std::ffi::{CStr, CString};

pub(crate) const MAX_ACTIVATE_DEPTH: usize = 36;

/// Splits pipe-delimited fallback action strings (`"a | b | c"`) into trimmed candidates.
pub(crate) fn split_action_candidates(act_str: &str) -> impl Iterator<Item = &str> {
    act_str.split('|').map(str::trim).filter(|s| !s.is_empty())
}

/// Splits a detailed action name into `(prefix, short_name)` with `"win"` default prefix.
pub(crate) fn split_action_prefix_and_name(full_name: &str) -> (&str, &str) {
    full_name.split_once('.').unwrap_or(("win", full_name))
}

unsafe fn try_activate_on_action_group(
    group_ptr: *mut c_void,
    short_name_cstr: &CStr,
    target_param: *mut GVariant,
    require_enabled: bool,
) -> bool {
    if group_ptr.is_null() {
        return false;
    }
    let has = unsafe { g_action_group_has_action(group_ptr, short_name_cstr.as_ptr()) }
        .map(|r| r != 0)
        .unwrap_or(false);
    if !has {
        return false;
    }
    if require_enabled {
        let is_enabled =
            unsafe { g_action_group_get_action_enabled(group_ptr, short_name_cstr.as_ptr()) }
                .map(|r| r != 0)
                .unwrap_or(true);
        if !is_enabled {
            return false;
        }
    }
    if !target_param.is_null() {
        unsafe {
            let _ = g_variant_ref(target_param);
        }
    }
    unsafe {
        let _ = g_action_group_activate_action(group_ptr, short_name_cstr.as_ptr(), target_param);
    }
    true
}

/// Dispatches an action string (supporting `|` fallback chains and detailed target syntax).
pub(crate) unsafe fn dispatch_widget_or_app_action(
    act_str: &str,
    target_param: *mut GVariant,
) -> bool {
    let window = HookState::active_window();
    let app = HookState::active_app();

    let mut live_widgets = HashSet::new();
    if !window.is_null() {
        unsafe {
            collect_live_widgets(window, 0, &mut live_widgets);
        }
    }
    let live_groups = HookState::get_live_action_groups(&live_widgets);

    for candidate in split_action_candidates(act_str) {
        let Some(parsed) = (unsafe { DetailedActionName::parse(candidate, target_param) }) else {
            continue;
        };

        let effective_name_ptr = parsed.name_ptr();
        let effective_target = parsed.target_ptr();
        let full_name = unsafe { parsed.full_name() };
        let mut handled = false;

        // 1. Direct lookup in intercepted live GActionGroups (e.g. "slot", "view", "path-bar", "toolbar")
        if let Some((prefix, short_name)) = full_name.split_once('.')
            && let Ok(c_short) = CString::new(short_name) {
                for (w_addr, grp_prefix, g_addr) in &live_groups {
                    if grp_prefix == prefix
                        && unsafe {
                            try_activate_on_action_group(
                                *g_addr as *mut c_void,
                                &c_short,
                                effective_target,
                                true,
                            )
                        }
                    {
                        hook_log!(
                            "  -> Activated '{full_name}' directly on GActionGroup '{prefix}' (widget={})",
                            unsafe { widget_type_name(*w_addr as *mut GtkWidget) }
                        );
                        handled = true;
                        break;
                    }
                }
            }

        // 2. Fallback: Walk the GTK4 widget tree (up to depth 36) using `gtk_widget_activate_action_variant`
        if !handled && !window.is_null() {
            handled =
                unsafe { activate_on_focused_or_window(window, effective_name_ptr, effective_target) };
            if handled {
                hook_log!("  -> Activated '{full_name}' via GtkWidget subtree muxer");
            }
        }

        // 3. Fallback: Check top-level GApplication and GtkWindow GActionGroups
        if !handled {
            let (prefix, short_name) = split_action_prefix_and_name(full_name);
            if let Ok(c_short) = CString::new(short_name) {
                let target_grp = if prefix == "app" { app } else { window };
                if unsafe {
                    try_activate_on_action_group(target_grp, &c_short, effective_target, false)
                } {
                    hook_log!("  -> Activated '{full_name}' on root '{prefix}' GActionGroup");
                    handled = true;
                }
            }
        }

        if handled {
            return true;
        }
        hook_log!("  -> Candidate '{full_name}' not found on any active widget/group");
    }
    false
}

unsafe fn activate_on_focused_or_window(
    window: *mut GtkWidget,
    action_ptr: *const c_char,
    target_param: *mut GVariant,
) -> bool {
    if let Some(focus_w) = unsafe { gtk_window_get_focus(window) }
        && !focus_w.is_null() {
            if !target_param.is_null() {
                unsafe {
                    let _ = g_variant_ref(target_param);
                }
            }
            let ok = unsafe {
                gtk_widget_activate_action_variant(focus_w, action_ptr, target_param)
            }
            .map(|r| r != 0)
            .unwrap_or(false);
            if ok {
                return true;
            }
        }

    unsafe { try_activate_on_subtree(window, 0, action_ptr, target_param) }
}

unsafe fn try_activate_on_subtree(
    widget: *mut GtkWidget,
    depth: usize,
    action_ptr: *const c_char,
    target_param: *mut GVariant,
) -> bool {
    if widget.is_null() || depth > MAX_ACTIVATE_DEPTH {
        return false;
    }

    let cname = unsafe { widget_type_name(widget) };
    if is_ignored_widget_class(&cname) {
        return false;
    }

    if !target_param.is_null() {
        unsafe {
            let _ = g_variant_ref(target_param);
        }
    }

    let activated = unsafe { gtk_widget_activate_action_variant(widget, action_ptr, target_param) }
        .map(|res| res != 0)
        .unwrap_or(false);
    if activated {
        return true;
    }

    for child in unsafe { get_widget_children(widget, depth == 0) } {
        if unsafe { try_activate_on_subtree(child, depth + 1, action_ptr, target_param) } {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{split_action_candidates, split_action_prefix_and_name};

    #[test]
    fn splits_fallback_candidates_and_prefixes_cleanly() {
        let candidates: Vec<&str> =
            split_action_candidates(" win.zoom-in | | app.zoom-in |zoom-in ").collect();
        assert_eq!(candidates, vec!["win.zoom-in", "app.zoom-in", "zoom-in"]);
        assert_eq!(split_action_prefix_and_name("app.quit"), ("app", "quit"));
        assert_eq!(
            split_action_prefix_and_name("slot.toggle-hidden"),
            ("slot", "toggle-hidden")
        );
        assert_eq!(split_action_prefix_and_name("copy"), ("win", "copy"));
    }
}
