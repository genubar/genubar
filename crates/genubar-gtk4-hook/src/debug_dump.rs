//! CLI diagnostic introspection dumper for live `GActionGroup`s and GTK widget hierarchies.

use crate::button_heuristics::{infer_button_title, is_ignored_widget_class};
use crate::ffi::{
    GApplication, GType, GtkWidget, g_action_group_get_action_enabled,
    g_action_group_get_action_state, g_action_group_list_actions, g_free, g_strfreev,
    g_type_check_instance_is_a, g_variant_print, g_variant_unref, gtk_menu_button_get_type,
    gtk_popover_menu_get_type, widget_type_name,
};
use crate::state::HookState;
use crate::widget_tree::{MAX_TREE_DEPTH, collect_live_widgets, get_widget_children};
use libc::c_void;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ptr;

pub unsafe fn dump_window_introspection(window: *mut GtkWidget, app: *mut GApplication) -> String {
    let mut out = String::new();
    if window.is_null() {
        return String::from("No active GTK window mapped in this process.\n");
    }

    let win_class = unsafe { widget_type_name(window) };
    out.push_str(&format!(
        "=== GTK Hook Introspection (Window={win_class} @ {window:?}) ===\n\n"
    ));

    let mut live_widgets = HashSet::new();
    unsafe {
        collect_live_widgets(window, 0, &mut live_widgets);
    }

    out.push_str("--- 1. Live GActionGroups & Actions ---\n");
    if !app.is_null() {
        unsafe {
            dump_single_action_group("app", "GApplication", app, &mut out);
        }
    }
    unsafe {
        dump_single_action_group("win", &win_class, window, &mut out);
    }

    for (w_addr, prefix, g_addr) in HookState::get_live_action_groups(&live_widgets) {
        let w_class = unsafe { widget_type_name(w_addr as *mut GtkWidget) };
        unsafe {
            dump_single_action_group(&prefix, &w_class, g_addr as *mut c_void, &mut out);
        }
    }

    out.push_str("\n--- 2. Widget Hierarchy (Depth 0..36) ---\n");
    let menu_btn_type = unsafe { gtk_menu_button_get_type() }.unwrap_or(0);
    let popover_type = unsafe { gtk_popover_menu_get_type() }.unwrap_or(0);
    let mut visited = HashSet::new();
    unsafe {
        dump_widget_tree_recursive(
            window,
            0,
            menu_btn_type,
            popover_type,
            &mut visited,
            &mut out,
        );
    }

    out
}

unsafe fn dump_single_action_group(
    prefix: &str,
    owner_class: &str,
    group_ptr: *mut c_void,
    out: &mut String,
) {
    let Some(list_ptr) = (unsafe { g_action_group_list_actions(group_ptr) }) else {
        return;
    };
    if list_ptr.is_null() {
        return;
    }

    let mut actions = Vec::new();
    let mut idx = 0usize;
    loop {
        let item_ptr = unsafe { *list_ptr.add(idx) };
        if item_ptr.is_null() {
            break;
        }
        if let Ok(name) = unsafe { CStr::from_ptr(item_ptr).to_str() } {
            let enabled = unsafe { g_action_group_get_action_enabled(group_ptr, item_ptr) }
                .map(|r| r != 0)
                .unwrap_or(false);
            let state_ptr = unsafe { g_action_group_get_action_state(group_ptr, item_ptr) }
                .unwrap_or(ptr::null_mut());
            let state_str = if !state_ptr.is_null() {
                let printed = unsafe { g_variant_print(state_ptr, 1) }.unwrap_or(ptr::null_mut());
                let s = if !printed.is_null() {
                    let txt = unsafe { CStr::from_ptr(printed).to_string_lossy().to_string() };
                    unsafe {
                        let _ = g_free(printed as *mut c_void);
                    }
                    format!(" state={txt}")
                } else {
                    String::new()
                };
                unsafe {
                    let _ = g_variant_unref(state_ptr);
                }
                s
            } else {
                String::new()
            };

            actions.push(format!(
                "  {prefix}.{name:<32} [enabled={enabled:<5}]{state_str}"
            ));
        }
        idx += 1;
    }
    unsafe {
        let _ = g_strfreev(list_ptr);
    }

    actions.sort();
    if !actions.is_empty() {
        out.push_str(&format!(
            "[Prefix '{prefix}' on {owner_class} ({} actions)]:\n{}\n",
            actions.len(),
            actions.join("\n")
        ));
    }
}

unsafe fn dump_widget_tree_recursive(
    widget: *mut GtkWidget,
    depth: usize,
    menu_btn_type: GType,
    popover_type: GType,
    visited: &mut HashSet<usize>,
    out: &mut String,
) {
    if widget.is_null() || depth > MAX_TREE_DEPTH || !visited.insert(widget as usize) {
        return;
    }
    let cname = unsafe { widget_type_name(widget) };
    let indent = "  ".repeat(depth);

    let is_mb = menu_btn_type != 0
        && unsafe { g_type_check_instance_is_a(widget, menu_btn_type) }
            .map(|r| r != 0)
            .unwrap_or(false);
    let is_pop = popover_type != 0
        && unsafe { g_type_check_instance_is_a(widget, popover_type) }
            .map(|r| r != 0)
            .unwrap_or(false);

    if is_mb {
        let title = unsafe { infer_button_title(widget) };
        out.push_str(&format!(
            "{indent}[depth={depth}] *{cname}* title='{title}'\n"
        ));
    } else if is_pop {
        out.push_str(&format!("{indent}[depth={depth}] *{cname}*\n"));
    } else if cname.contains("Nautilus")
        || cname.contains("Header")
        || cname.contains("Toolbar")
        || cname.contains("PathBar")
    {
        out.push_str(&format!("{indent}[depth={depth}] {cname}\n"));
    }

    if is_ignored_widget_class(&cname) {
        return;
    }

    for child in unsafe { get_widget_children(widget, depth == 0) } {
        unsafe {
            dump_widget_tree_recursive(child, depth + 1, menu_btn_type, popover_type, visited, out);
        }
    }
}
