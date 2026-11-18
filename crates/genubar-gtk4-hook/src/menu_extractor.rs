//! Builds an aggregated `GMenu` from all visible `GtkMenuButton` widgets in a window.

use crate::button_heuristics::{
    infer_button_title, is_ignored_widget_class, should_hide_original_button,
};
use crate::ffi::{
    GApplication, GMenu, GMenuModel, GType, GtkWidget, g_menu_append_section,
    g_menu_append_submenu, g_menu_model_get_n_items, g_menu_new, g_type_check_instance_is_a,
    gtk_application_get_menubar, gtk_application_window_get_type,
    gtk_application_window_set_show_menubar, gtk_menu_button_get_menu_model,
    gtk_menu_button_get_popover, gtk_popover_menu_get_menu_model, gtk_popover_menu_get_type,
    gtk_widget_get_mapped, gtk_widget_get_visible, gtk_widget_set_visible, widget_type_name,
};
use crate::hook_log;
use crate::state::HookState;
use crate::widget_tree::{MAX_TREE_DEPTH, get_widget_children};
use std::collections::HashSet;
use std::ffi::CString;
use std::ptr;

pub type ExportSignatureItem = (String, i32, usize);

pub struct MenuExtractor;

impl MenuExtractor {
    pub unsafe fn maybe_hide_app_window_menubar(window: *mut GtkWidget, app: *mut GApplication) {
        if window.is_null() || app.is_null() || !should_hide_original_button() {
            return;
        }
        let app_menubar =
            unsafe { gtk_application_get_menubar(app) }.unwrap_or(ptr::null_mut());
        if app_menubar.is_null() {
            return;
        }
        let n_items = unsafe { g_menu_model_get_n_items(app_menubar) }.unwrap_or(0);
        if n_items <= 0 {
            return;
        }
        let app_win_type = unsafe { gtk_application_window_get_type() }.unwrap_or(0);
        let is_app_win = app_win_type != 0
            && unsafe { g_type_check_instance_is_a(window, app_win_type) }
                .map(|r| r != 0)
                .unwrap_or(false);
        if is_app_win {
            unsafe {
                let _ = gtk_application_window_set_show_menubar(window, 0);
            }
        }
    }

    pub unsafe fn build_master_menu_for_window(
        window: *mut GtkWidget,
        app: *mut GApplication,
        menu_btn_type: GType,
    ) -> (*mut GMenu, usize, Vec<ExportSignatureItem>) {
        let Some(master_menu) = (unsafe { g_menu_new() }) else {
            return (ptr::null_mut(), 0, Vec::new());
        };
        if master_menu.is_null() {
            return (ptr::null_mut(), 0, Vec::new());
        }

        if !app.is_null() {
            let app_menubar =
                unsafe { gtk_application_get_menubar(app) }.unwrap_or(ptr::null_mut());
            if !app_menubar.is_null() {
                let n_items = unsafe { g_menu_model_get_n_items(app_menubar) }.unwrap_or(0);
                if n_items > 0 {
                    unsafe {
                        Self::maybe_hide_app_window_menubar(window, app);
                        let _ = g_menu_append_section(master_menu, ptr::null(), app_menubar);
                    }
                    let sig = vec![(String::from("AppMenubar"), n_items, app_menubar as usize)];
                    hook_log!(
                        "Extracted native GtkApplication menubar on {} ({:?}): {} top-level items",
                        unsafe { widget_type_name(window) },
                        window,
                        n_items
                    );
                    return (master_menu, n_items as usize, sig);
                }
            }
        }

        let popover_menu_type = unsafe { gtk_popover_menu_get_type() }.unwrap_or(0);
        let window_mapped = unsafe { gtk_widget_get_mapped(window) }.unwrap_or(0) != 0;
        let mut found_count = 0usize;
        let mut seen_models = HashSet::new();
        let mut visited_widgets = HashSet::new();
        let mut signature: Vec<ExportSignatureItem> = Vec::new();

        unsafe {
            Self::walk_tree_recursive(
                window,
                window,
                0,
                window_mapped,
                menu_btn_type,
                popover_menu_type,
                master_menu,
                &mut found_count,
                &mut seen_models,
                &mut visited_widgets,
                &mut signature,
            );
        }

        // Preserve any primary menu model previously found on this window when collapsible sidebars unmap it.
        if found_count > 0 {
            for (title, model_ptr) in HookState::get_window_primary_models(window) {
                let model_addr = model_ptr as usize;
                if seen_models.insert(model_addr) {
                    let g_model = model_ptr as *mut GMenuModel;
                    let n_items = unsafe { g_menu_model_get_n_items(g_model) }.unwrap_or(0);
                    if n_items > 0
                        && let Ok(c_title) = CString::new(title.clone()) {
                            unsafe {
                                let _ = g_menu_append_submenu(master_menu, c_title.as_ptr(), g_model);
                            }
                            found_count += 1;
                            signature.push((title, n_items, model_addr));
                        }
                }
            }
        }

        hook_log!(
            "Scan complete on {} ({:?}): extracted {} GMenuModel section(s)",
            unsafe { widget_type_name(window) },
            window,
            found_count
        );

        (master_menu, found_count, signature)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn walk_tree_recursive(
        root_window: *mut GtkWidget,
        widget: *mut GtkWidget,
        depth: usize,
        window_mapped: bool,
        menu_btn_type: GType,
        popover_menu_type: GType,
        master_menu: *mut GMenu,
        found_count: &mut usize,
        seen_models: &mut HashSet<usize>,
        visited_widgets: &mut HashSet<usize>,
        signature: &mut Vec<ExportSignatureItem>,
    ) {
        if widget.is_null() || depth > MAX_TREE_DEPTH || !visited_widgets.insert(widget as usize) {
            return;
        }

        let class_name = unsafe { widget_type_name(widget) };
        if is_ignored_widget_class(&class_name) {
            return;
        }

        let is_menu_btn = unsafe { g_type_check_instance_is_a(widget, menu_btn_type) }
            .map(|r| r != 0)
            .unwrap_or(false);

        if depth > 0 {
            let is_vis = unsafe { gtk_widget_get_visible(widget) }.unwrap_or(1) != 0;
            let is_map = !window_mapped
                || unsafe { gtk_widget_get_mapped(widget) }.unwrap_or(1) != 0;

            if (!is_vis || !is_map) && !(is_menu_btn && HookState::was_hidden_by_hook(widget)) {
                return;
            }
        }

        if is_menu_btn {
            unsafe {
                Self::process_menu_button(
                    root_window,
                    widget,
                    depth,
                    window_mapped,
                    popover_menu_type,
                    master_menu,
                    found_count,
                    seen_models,
                    signature,
                );
            }
            return;
        }

        for child in unsafe { get_widget_children(widget, depth == 0) } {
            unsafe {
                Self::walk_tree_recursive(
                    root_window,
                    child,
                    depth + 1,
                    window_mapped,
                    menu_btn_type,
                    popover_menu_type,
                    master_menu,
                    found_count,
                    seen_models,
                    visited_widgets,
                    signature,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn process_menu_button(
        root_window: *mut GtkWidget,
        widget: *mut GtkWidget,
        depth: usize,
        window_mapped: bool,
        popover_menu_type: GType,
        master_menu: *mut GMenu,
        found_count: &mut usize,
        seen_models: &mut HashSet<usize>,
        signature: &mut Vec<ExportSignatureItem>,
    ) {
        let mut model: *mut GMenuModel =
            unsafe { gtk_menu_button_get_menu_model(widget) }.unwrap_or(ptr::null_mut());

        if model.is_null()
            && let Some(popover) = unsafe { gtk_menu_button_get_popover(widget) }
                && !popover.is_null() {
                    model = HookState::get_popover_menu_model(popover);

                    if model.is_null() && popover_menu_type != 0 {
                        let is_pop =
                            unsafe { g_type_check_instance_is_a(popover, popover_menu_type) }
                                .map(|r| r != 0)
                                .unwrap_or(false);
                        if is_pop {
                            model = unsafe { gtk_popover_menu_get_menu_model(popover) }
                                .unwrap_or(ptr::null_mut());
                        }
                    }
                }

        if model.is_null() || !seen_models.insert(model as usize) {
            return;
        }

        let n_items = unsafe { g_menu_model_get_n_items(model) }.unwrap_or(0);
        if n_items <= 0 {
            return;
        }

        let title = unsafe { infer_button_title(widget) };
        hook_log!(
            "  + GtkMenuButton at depth={depth}: title='{title}', items={n_items}, model={model:?}"
        );

        if title == "Options" || title == "File" {
            unsafe {
                HookState::record_window_primary_model(root_window, &title, model);
            }
        }

        if let Ok(c_title) = CString::new(title.clone()) {
            unsafe {
                let _ = g_menu_append_submenu(master_menu, c_title.as_ptr(), model);
                if window_mapped
                    && should_hide_original_button()
                    && (title == "Options" || title == "File")
                {
                    HookState::mark_hidden_by_hook(widget);
                    let _ = gtk_widget_set_visible(widget, 0);
                }
            }
            *found_count += 1;
            signature.push((title, n_items, model as usize));
        }
    }
}
