//! Per-widget maps (`GActionGroup` scopes, GTK3 popovers, hidden buttons, stacks) with weak-ref cleanup.

use super::{export_registry, window_context};
use crate::ffi::{GtkWidget, g_object_weak_ref};
use libc::c_void;
use std::collections::{HashMap, HashSet};
use std::ptr;
use std::sync::{Mutex, OnceLock};

static TRACKED_WIDGET_LIFETIMES: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
static TRACKED_ACTION_GROUPS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
static WIDGET_ACTION_GROUPS: OnceLock<Mutex<HashMap<(usize, String), usize>>> = OnceLock::new();
static POPOVER_MENU_MODELS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static HIDDEN_BY_HOOK_BUTTONS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
static CONNECTED_STACKS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
static WIDGET_CLASS_ACTION_ENABLED: OnceLock<Mutex<HashMap<(usize, String), bool>>> =
    OnceLock::new();

fn tracked_lifetimes_set() -> &'static Mutex<HashSet<usize>> {
    TRACKED_WIDGET_LIFETIMES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn tracked_action_groups_set() -> &'static Mutex<HashSet<usize>> {
    TRACKED_ACTION_GROUPS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn action_groups_map() -> &'static Mutex<HashMap<(usize, String), usize>> {
    WIDGET_ACTION_GROUPS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn popover_models_map() -> &'static Mutex<HashMap<usize, usize>> {
    POPOVER_MENU_MODELS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hidden_buttons_set() -> &'static Mutex<HashSet<usize>> {
    HIDDEN_BY_HOOK_BUTTONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn connected_stacks_set() -> &'static Mutex<HashSet<usize>> {
    CONNECTED_STACKS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn widget_class_action_map() -> &'static Mutex<HashMap<(usize, String), bool>> {
    WIDGET_CLASS_ACTION_ENABLED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `GWeakNotify` invoked by GObject when a tracked widget or window is finalized.
/// Removes the stale address from every global table to prevent memory leaks and address-reuse UB.
unsafe extern "C" fn on_widget_weak_notify(_data: *mut c_void, where_the_object_was: *mut c_void) {
    let addr = where_the_object_was as usize;
    if addr == 0 {
        return;
    }
    cleanup_destroyed_widget(addr);
    export_registry::release_window_primary_models(addr);
    window_context::remove_window_state(addr, where_the_object_was as *mut GtkWidget);
}

/// Attaches a single GObject weak reference to `widget` so its address is evicted upon destruction.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn track_widget_lifetime(widget: *mut GtkWidget) {
    if widget.is_null() {
        return;
    }
    let addr = widget as usize;
    let inserted = tracked_lifetimes_set()
        .lock()
        .map(|mut guard| guard.insert(addr))
        .unwrap_or(false);

    if inserted {
        unsafe {
            let _ = g_object_weak_ref(widget, on_widget_weak_notify, ptr::null_mut());
        }
    }
}

/// Evicts all state entries associated with `widget_addr` across every global tracking table.
pub fn cleanup_destroyed_widget(widget_addr: usize) {
    if widget_addr == 0 {
        return;
    }
    if let Ok(mut guard) = tracked_lifetimes_set().lock() {
        guard.remove(&widget_addr);
    }
    if let Ok(mut guard) = action_groups_map().lock() {
        guard.retain(|(w, _), _| *w != widget_addr);
    }
    if let Ok(mut guard) = widget_class_action_map().lock() {
        guard.retain(|(w, _), _| *w != widget_addr);
    }
    if let Ok(mut guard) = popover_models_map().lock() {
        guard.remove(&widget_addr);
    }
    if let Ok(mut guard) = hidden_buttons_set().lock() {
        guard.remove(&widget_addr);
    }
    if let Ok(mut guard) = connected_stacks_set().lock() {
        guard.remove(&widget_addr);
    }
}

unsafe extern "C" fn on_action_group_weak_notify(_data: *mut c_void, where_the_object_was: *mut c_void) {
    let addr = where_the_object_was as usize;
    if addr != 0
        && let Ok(mut guard) = tracked_action_groups_set().lock() {
            guard.remove(&addr);
        }
}

/// Attaches a single weak ref to a `GActionGroup` instance and returns `true` the first time it is seen.
///
/// # Safety
/// `group` must either be null or a valid `GObject` pointer on the GTK main thread.
pub unsafe fn mark_action_group_observed(group: *mut c_void) -> bool {
    if group.is_null() {
        return false;
    }
    let addr = group as usize;
    let inserted = tracked_action_groups_set()
        .lock()
        .map(|mut guard| guard.insert(addr))
        .unwrap_or(false);
    if inserted {
        unsafe {
            let _ = g_object_weak_ref(group, on_action_group_weak_notify, ptr::null_mut());
        }
    }
    inserted
}

pub fn mark_stack_connected(stack: *mut GtkWidget) -> bool {
    if stack.is_null() {
        return false;
    }
    track_widget_lifetime(stack);
    connected_stacks_set()
        .lock()
        .map(|mut guard| guard.insert(stack as usize))
        .unwrap_or(false)
}

pub(crate) fn set_widget_action_group_entry(
    widget_addr: usize,
    prefix: &str,
    group_addr: usize,
) -> bool {
    if widget_addr == 0 || prefix.is_empty() {
        return false;
    }
    let changed = action_groups_map()
        .lock()
        .map(|mut guard| {
            let key = (widget_addr, prefix.to_string());
            if group_addr == 0 {
                guard.remove(&key).is_some()
            } else {
                guard.insert(key, group_addr) != Some(group_addr)
            }
        })
        .unwrap_or(false);
    if changed {
        super::export_registry::bump_action_state_epoch();
    }
    changed
}

pub fn record_widget_action_group(widget: *mut GtkWidget, prefix: &str, group: *mut c_void) -> bool {
    if widget.is_null() || prefix.is_empty() {
        return false;
    }
    if !group.is_null() {
        track_widget_lifetime(widget);
    }
    set_widget_action_group_entry(widget as usize, prefix, group as usize)
}

pub fn get_live_action_groups(live_widgets: &HashSet<usize>) -> Vec<(usize, String, usize)> {
    let mut out = Vec::new();
    if let Ok(guard) = action_groups_map().lock() {
        for ((w_addr, prefix), g_addr) in guard.iter() {
            if live_widgets.contains(w_addr) {
                out.push((*w_addr, prefix.clone(), *g_addr));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

pub fn record_popover_menu_model(popover: *mut GtkWidget, model: *mut c_void) {
    if popover.is_null() {
        return;
    }
    if !model.is_null() {
        track_widget_lifetime(popover);
    }
    if let Ok(mut guard) = popover_models_map().lock() {
        if model.is_null() {
            guard.remove(&(popover as usize));
        } else {
            guard.insert(popover as usize, model as usize);
        }
    }
}

pub fn get_popover_menu_model(popover: *mut GtkWidget) -> *mut c_void {
    if popover.is_null() {
        return ptr::null_mut();
    }
    if let Ok(guard) = popover_models_map().lock()
        && let Some(&addr) = guard.get(&(popover as usize)) {
            return addr as *mut c_void;
        }
    ptr::null_mut()
}

pub fn mark_hidden_by_hook(widget: *mut GtkWidget) {
    if widget.is_null() {
        return;
    }
    track_widget_lifetime(widget);
    if let Ok(mut guard) = hidden_buttons_set().lock() {
        guard.insert(widget as usize);
    }
}

pub fn was_hidden_by_hook(widget: *mut GtkWidget) -> bool {
    if widget.is_null() {
        return false;
    }
    hidden_buttons_set()
        .lock()
        .map(|guard| guard.contains(&(widget as usize)))
        .unwrap_or(false)
}

pub(crate) fn set_widget_class_action_entry(
    widget_addr: usize,
    action_name: &str,
    enabled: bool,
) -> bool {
    if widget_addr == 0 || action_name.is_empty() {
        return false;
    }
    let changed = widget_class_action_map()
        .lock()
        .map(|mut guard| {
            let key = (widget_addr, action_name.to_string());
            guard.insert(key, enabled) != Some(enabled)
        })
        .unwrap_or(false);
    if changed {
        super::export_registry::bump_action_state_epoch();
    }
    changed
}

pub fn record_widget_class_action_enabled(
    widget: *mut GtkWidget,
    action_name: &str,
    enabled: bool,
) -> bool {
    if widget.is_null() || action_name.is_empty() {
        return false;
    }
    track_widget_lifetime(widget);
    set_widget_class_action_entry(widget as usize, action_name, enabled)
}

pub fn get_live_class_action_states(live_widgets: &HashSet<usize>) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    if let Ok(guard) = widget_class_action_map().lock() {
        for ((w_addr, action_name), enabled) in guard.iter() {
            if live_widgets.contains(w_addr) {
                out.push((action_name.clone(), *enabled));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
pub(crate) fn insert_test_entries_for_addr(addr: usize) {
    if let Ok(mut g) = action_groups_map().lock() {
        g.insert((addr, String::from("slot")), 0xDEAD);
    }
    if let Ok(mut g) = popover_models_map().lock() {
        g.insert(addr, 0xBEEF);
    }
    if let Ok(mut g) = hidden_buttons_set().lock() {
        g.insert(addr);
    }
    if let Ok(mut g) = connected_stacks_set().lock() {
        g.insert(addr);
    }
    if let Ok(mut g) = widget_class_action_map().lock() {
        g.insert((addr, String::from("win.copy")), true);
    }
}

#[cfg(test)]
pub(crate) fn has_any_entries_for_addr(addr: usize) -> bool {
    let live = HashSet::from([addr]);
    !get_live_action_groups(&live).is_empty()
        || !get_popover_menu_model(addr as *mut GtkWidget).is_null()
        || was_hidden_by_hook(addr as *mut GtkWidget)
        || !get_live_class_action_states(&live).is_empty()
        || connected_stacks_set()
            .lock()
            .map(|g| g.contains(&addr))
            .unwrap_or(false)
}
