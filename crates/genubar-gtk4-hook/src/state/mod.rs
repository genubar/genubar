//! Facade over process-local window context, D-Bus export registry, and widget tracking state.

pub mod export_registry;
pub mod widget_tracking;
pub mod window_context;

use crate::ffi::{GApplication, GtkWidget};
use libc::c_void;
use std::collections::HashSet;

pub struct HookState;

impl HookState {
    pub fn set_active_context(window: *mut GtkWidget, app: *mut GApplication) {
        window_context::set_active_context(window, app);
    }

    pub fn record_menu_window(window: *mut GtkWidget) {
        window_context::record_menu_window(window);
    }

    pub fn remove_destroyed_window(window: *mut GtkWidget) {
        if window.is_null() {
            return;
        }
        let win_addr = window as usize;
        widget_tracking::cleanup_destroyed_widget(win_addr);
        export_registry::release_window_primary_models(win_addr);
        export_registry::invalidate_export_signature();
        window_context::remove_window_state(win_addr, window);
    }

    pub fn mark_stack_connected(stack: *mut GtkWidget) -> bool {
        widget_tracking::mark_stack_connected(stack)
    }

    /// # Safety
    /// `group` must either be null or a valid `GActionGroup` pointer on the GTK thread.
    pub unsafe fn mark_action_group_observed(group: *mut c_void) -> bool {
        unsafe { widget_tracking::mark_action_group_observed(group) }
    }

    pub fn bump_action_state_epoch() -> u64 {
        export_registry::bump_action_state_epoch()
    }

    pub fn active_window() -> *mut GtkWidget {
        window_context::active_window()
    }

    pub fn active_app() -> *mut GApplication {
        window_context::active_app()
    }

    pub fn zoom_percent() -> i32 {
        window_context::zoom_percent()
    }

    pub fn set_zoom_percent(pct: i32) {
        window_context::set_zoom_percent(pct);
    }

    pub fn adjust_zoom(delta: i32) {
        window_context::adjust_zoom(delta);
    }

    pub fn swap_menu_export_id(new_id: u32) -> u32 {
        export_registry::swap_menu_export_id(new_id)
    }

    pub fn swap_app_action_export_id(new_id: u32) -> u32 {
        export_registry::swap_app_action_export_id(new_id)
    }

    pub fn swap_win_action_export_id(new_id: u32) -> u32 {
        export_registry::swap_win_action_export_id(new_id)
    }

    pub fn bridge_export_id() -> u32 {
        export_registry::bridge_export_id()
    }

    pub fn set_bridge_export_id(id: u32) {
        export_registry::set_bridge_export_id(id);
    }

    pub fn record_widget_action_group(
        widget: *mut GtkWidget,
        prefix: &str,
        group: *mut c_void,
    ) -> bool {
        widget_tracking::record_widget_action_group(widget, prefix, group)
    }

    pub fn get_live_action_groups(live_widgets: &HashSet<usize>) -> Vec<(usize, String, usize)> {
        widget_tracking::get_live_action_groups(live_widgets)
    }

    pub fn record_popover_menu_model(popover: *mut GtkWidget, model: *mut c_void) {
        widget_tracking::record_popover_menu_model(popover, model);
    }

    pub fn get_popover_menu_model(popover: *mut GtkWidget) -> *mut c_void {
        widget_tracking::get_popover_menu_model(popover)
    }

    pub fn mark_hidden_by_hook(widget: *mut GtkWidget) {
        widget_tracking::mark_hidden_by_hook(widget);
    }

    pub fn was_hidden_by_hook(widget: *mut GtkWidget) -> bool {
        widget_tracking::was_hidden_by_hook(widget)
    }

    /// # Safety
    /// `window` and `model` must either be null or valid GObject pointers on the GTK thread.
    pub unsafe fn record_window_primary_model(
        window: *mut GtkWidget,
        title: &str,
        model: *mut c_void,
    ) {
        unsafe { export_registry::record_window_primary_model(window, title, model) };
    }

    pub fn get_window_primary_models(window: *mut GtkWidget) -> Vec<(String, *mut c_void)> {
        export_registry::get_window_primary_models(window)
    }

    pub fn record_widget_class_action_enabled(
        widget: *mut GtkWidget,
        action_name: &str,
        enabled: bool,
    ) -> bool {
        widget_tracking::record_widget_class_action_enabled(widget, action_name, enabled)
    }

    pub fn get_live_class_action_states(live_widgets: &HashSet<usize>) -> Vec<(String, bool)> {
        widget_tracking::get_live_class_action_states(live_widgets)
    }

    pub fn check_and_update_export_signature(
        window: *mut GtkWidget,
        sig: &[(String, i32, usize)],
    ) -> bool {
        export_registry::check_and_update_export_signature(window, sig)
    }

    pub fn invalidate_export_signature() {
        export_registry::invalidate_export_signature();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_destroyed_widget_purges_all_tracking_tables() {
        let fake_addr = 0x1234_5678usize;
        widget_tracking::insert_test_entries_for_addr(fake_addr);
        assert!(widget_tracking::has_any_entries_for_addr(fake_addr));

        widget_tracking::cleanup_destroyed_widget(fake_addr);
        assert!(!widget_tracking::has_any_entries_for_addr(fake_addr));
    }

    #[test]
    fn window_zoom_clamps_to_safe_bounds_and_resets_cleanly() {
        let fake_win = 0x9999_0000usize;
        window_context::set_zoom_percent_for_addr(fake_win, 1000);
        assert_eq!(window_context::zoom_percent_for_addr(fake_win), 500);

        window_context::adjust_zoom_for_addr(fake_win, 0);
        assert_eq!(window_context::zoom_percent_for_addr(fake_win), 100);

        window_context::set_zoom_percent_for_addr(fake_win, 5);
        assert_eq!(window_context::zoom_percent_for_addr(fake_win), 20);
        window_context::remove_window_state(fake_win, std::ptr::null_mut());
    }

    #[test]
    fn action_group_or_state_changes_invalidate_export_signature_epoch() {
        let fake_win = 0x7777_0000 as *mut GtkWidget;
        let fake_sig = vec![(String::from("View Options"), 5, 0x1111)];
        export_registry::invalidate_export_signature();
        assert!(export_registry::check_and_update_export_signature(
            fake_win, &fake_sig
        ));
        assert!(!export_registry::check_and_update_export_signature(
            fake_win, &fake_sig
        ));

        assert!(widget_tracking::set_widget_action_group_entry(
            fake_win as usize,
            "view",
            0x2222
        ));
        assert!(export_registry::check_and_update_export_signature(
            fake_win, &fake_sig
        ));
        assert!(!widget_tracking::set_widget_action_group_entry(
            fake_win as usize,
            "view",
            0x2222
        ));

        assert!(widget_tracking::set_widget_class_action_entry(
            fake_win as usize,
            "win.copy",
            false
        ));
        assert!(export_registry::check_and_update_export_signature(
            fake_win, &fake_sig
        ));

        widget_tracking::cleanup_destroyed_widget(fake_win as usize);
        export_registry::invalidate_export_signature();
    }
}
