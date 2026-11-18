//! Unified GTK4 / GTK3 / Libadwaita widget tree walker.

use crate::ffi::{
    GType, GtkWidget, adw_view_stack_get_type, adw_view_stack_get_visible_child,
    g_signal_connect_data, g_type_check_instance_is_a, gtk_container_forall,
    gtk_container_get_type, gtk_stack_get_type, gtk_stack_get_visible_child,
    gtk_widget_get_first_child, gtk_widget_get_mapped, gtk_widget_get_next_sibling,
    gtk_widget_get_visible, gtk_window_get_titlebar, widget_type_name,
};
use crate::scanner::trigger_debounced_rescans;
use crate::state::HookState;
use libc::c_void;
use std::collections::HashSet;
use std::ptr;

pub const MAX_TREE_DEPTH: usize = 36;

unsafe extern "C" fn collect_gtk3_child_cb(child: *mut GtkWidget, user_data: *mut c_void) {
    if child.is_null() || user_data.is_null() {
        return;
    }
    let vec = unsafe { &mut *(user_data as *mut Vec<*mut GtkWidget>) };
    if !vec.contains(&child) {
        vec.push(child);
    }
}

unsafe extern "C" fn on_stack_visible_child_notify(
    _stack: *mut GtkWidget,
    _pspec: *mut c_void,
    _user_data: *mut c_void,
) {
    unsafe {
        trigger_debounced_rescans();
    }
}

unsafe fn try_resolve_stack_children(
    widget: *mut GtkWidget,
    stack_type: GType,
    get_visible_fn: unsafe fn(*mut GtkWidget) -> Option<*mut GtkWidget>,
) -> Option<Vec<*mut GtkWidget>> {
    if stack_type == 0 {
        return None;
    }
    let is_stack = unsafe { g_type_check_instance_is_a(widget, stack_type) }
        .map(|r| r != 0)
        .unwrap_or(false);
    if !is_stack {
        return None;
    }
    if HookState::mark_stack_connected(widget) {
        unsafe {
            let _ = g_signal_connect_data(
                widget,
                c"notify::visible-child".as_ptr(),
                on_stack_visible_child_notify as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
        }
    }
    let mut children = Vec::new();
    if let Some(vis_child) = unsafe { get_visible_fn(widget) }
        && !vis_child.is_null() {
            children.push(vis_child);
        }
    Some(children)
}

unsafe fn resolve_stack_children_if_applicable(widget: *mut GtkWidget) -> Option<Vec<*mut GtkWidget>> {
    if let Some(gtk_stack_t) = unsafe { gtk_stack_get_type() } {
        unsafe fn get_gtk_vis(w: *mut GtkWidget) -> Option<*mut GtkWidget> {
            unsafe { gtk_stack_get_visible_child(w) }
        }
        if let Some(children) =
            unsafe { try_resolve_stack_children(widget, gtk_stack_t, get_gtk_vis) }
        {
            return Some(children);
        }
    }
    if let Some(adw_stack_t) = unsafe { adw_view_stack_get_type() } {
        unsafe fn get_adw_vis(w: *mut GtkWidget) -> Option<*mut GtkWidget> {
            unsafe { adw_view_stack_get_visible_child(w) }
        }
        if let Some(children) =
            unsafe { try_resolve_stack_children(widget, adw_stack_t, get_adw_vis) }
        {
            return Some(children);
        }
    }
    None
}

/// Returns direct child widgets on both GTK4 (`gtk_widget_get_first_child`) and GTK3 (`gtk_container_forall`).
/// For `GtkStack` and `AdwViewStack`, returns only the currently visible child so inactive stack pages are skipped.
pub unsafe fn get_widget_children(widget: *mut GtkWidget, is_root_window: bool) -> Vec<*mut GtkWidget> {
    let mut children = Vec::new();
    if widget.is_null() {
        return children;
    }

    if let Some(stack_children) = unsafe { resolve_stack_children_if_applicable(widget) } {
        return stack_children;
    }

    if let Some(first) = unsafe { gtk_widget_get_first_child(widget) } {
        let mut cur = first;
        while !cur.is_null() {
            children.push(cur);
            cur = unsafe { gtk_widget_get_next_sibling(cur) }.unwrap_or(ptr::null_mut());
        }
        return children;
    }

    if is_root_window
        && let Some(titlebar) = unsafe { gtk_window_get_titlebar(widget) }
            && !titlebar.is_null() {
                children.push(titlebar);
            }

    if let Some(container_type) = unsafe { gtk_container_get_type() } {
        let is_container = container_type != 0
            && unsafe { g_type_check_instance_is_a(widget, container_type) }
                .map(|r| r != 0)
                .unwrap_or(false);
        if is_container {
            unsafe {
                let _ = gtk_container_forall(
                    widget,
                    collect_gtk3_child_cb,
                    &mut children as *mut Vec<*mut GtkWidget> as *mut c_void,
                );
            }
        }
    }

    children
}

pub unsafe fn collect_live_widgets(
    widget: *mut GtkWidget,
    depth: usize,
    out: &mut HashSet<usize>,
) {
    if widget.is_null() || depth > MAX_TREE_DEPTH {
        return;
    }
    if depth > 0 && !HookState::was_hidden_by_hook(widget) {
        let is_vis = unsafe { gtk_widget_get_visible(widget) }.unwrap_or(1) != 0;
        let is_map = unsafe { gtk_widget_get_mapped(widget) }.unwrap_or(1) != 0;
        if !is_vis || !is_map {
            return;
        }
    }
    if !out.insert(widget as usize) {
        return;
    }
    let cname = unsafe { widget_type_name(widget) };
    if matches!(
        cname.as_str(),
        "GtkGridView" | "GtkListView" | "GtkColumnView" | "GtkTreeView" | "GtkTextView"
    ) {
        return;
    }
    for child in unsafe { get_widget_children(widget, depth == 0) } {
        unsafe {
            collect_live_widgets(child, depth + 1, out);
        }
    }
}
