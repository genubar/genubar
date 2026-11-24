use crate::error::UiError;
use crate::state::AppState;
use crate::views::{
    about_view::build_about_view, bundles_view::build_bundles_view,
    general_view::build_general_view, icons_view::build_icons_view,
    installed_view::build_installed_view, marketplace_view::build_marketplace_view,
    patches_view::build_patches_view, profiles_view::build_profiles_view,
    protocols_view::build_protocols_view, styles_view::build_styles_view,
    system_menus_view::build_system_menus_view,
};
use genubar_config::ConfigManager;
use genubar_patcher::PatchManager;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Wraps a stateless settings view in a persistent `gtk::Box` slot so only that tab refreshes when dirty.
fn add_refreshable_slot_tab<F>(
    stack: &gtk::Stack,
    state: &AppState,
    tab_id: &str,
    title: &str,
    builder: F,
) where
    F: Fn(&AppState) -> gtk::Widget + 'static,
{
    let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    slot.set_vexpand(true);
    slot.set_hexpand(true);
    slot.append(&builder(state));
    stack.add_titled(&slot, Some(tab_id), title);

    let st = state.clone();
    let slot_c = slot.clone();
    let builder_rc = Rc::new(builder);
    state.register_tab_refresher(
        tab_id,
        Rc::new(move || {
            while let Some(child) = slot_c.first_child() {
                slot_c.remove(&child);
            }
            slot_c.append(&builder_rc(&st));
        }),
    );
}

pub fn build_main_window(
    app: &adw::Application,
    recommend_filter: Option<String>,
) -> Result<adw::ApplicationWindow, UiError> {
    let cfg = ConfigManager::load_or_bootstrap()?;
    let patcher = PatchManager::load_or_init()?;

    let toast_overlay = adw::ToastOverlay::new();
    let view_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hexpand(true)
        .vexpand(true)
        .build();

    let status_spinner = gtk::Spinner::builder()
        .visible(false)
        .valign(gtk::Align::Center)
        .build();
    let status_icon = gtk::Image::builder()
        .icon_name("object-select-symbolic")
        .visible(true)
        .valign(gtk::Align::Center)
        .build();
    let status_label = gtk::Label::builder()
        .label("All changes applied")
        .css_classes(["dim-label"])
        .valign(gtk::Align::Center)
        .build();

    let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    status_box.set_margin_start(10);
    status_box.set_margin_end(10);
    status_box.append(&status_spinner);
    status_box.append(&status_icon);
    status_box.append(&status_label);

    let state = AppState {
        config: Rc::new(RefCell::new(cfg)),
        patcher: Rc::new(RefCell::new(patcher)),
        toast_overlay: toast_overlay.clone(),
        view_stack: view_stack.clone(),
        debounce_timer: Rc::new(RefCell::new(None)),
        status_spinner,
        status_icon,
        status_label,
        tab_refreshers: Rc::new(RefCell::new(HashMap::new())),
        dirty_tabs: Rc::new(RefCell::new(HashSet::new())),
        registry_cache: Rc::new(RefCell::new(Default::default())),
    };

    let has_rec_filter = recommend_filter.is_some();

    add_refreshable_slot_tab(
        &view_stack,
        &state,
        "general",
        "General",
        build_general_view,
    );
    view_stack.add_titled(
        &build_styles_view(&state),
        Some("styles"),
        "Styles & Appearance",
    );
    view_stack.add_titled(&build_icons_view(&state), Some("icons"), "Icons & Packs");
    add_refreshable_slot_tab(
        &view_stack,
        &state,
        "system_menus",
        "System Menus & Sorter",
        build_system_menus_view,
    );
    view_stack.add_titled(
        &build_profiles_view(&state),
        Some("profiles"),
        "App Profiles",
    );
    view_stack.add_titled(
        &build_patches_view(&state, recommend_filter),
        Some("patches"),
        "Patches",
    );
    add_refreshable_slot_tab(
        &view_stack,
        &state,
        "protocols",
        "Protocols & Cache",
        build_protocols_view,
    );
    view_stack.add_titled(
        &build_installed_view(&state),
        Some("installed"),
        "Installed Packages",
    );
    view_stack.add_titled(&build_bundles_view(&state), Some("bundles"), "Bundles");
    view_stack.add_titled(
        &build_marketplace_view(&state),
        Some("marketplace"),
        "Marketplace",
    );
    add_refreshable_slot_tab(
        &view_stack,
        &state,
        "about",
        "About & Updates",
        build_about_view,
    );

    if has_rec_filter {
        view_stack.set_visible_child_name("patches");
    } else {
        view_stack.set_visible_child_name("general");
    }

    {
        let st = state.clone();
        view_stack.connect_visible_child_name_notify(move |stack| {
            if let Some(name) = stack.visible_child_name() {
                st.refresh_tab_if_dirty(name.as_str());
            }
        });
    }

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    sidebar_box.set_width_request(235);
    sidebar_box.set_margin_top(8);
    sidebar_box.set_margin_bottom(8);
    sidebar_box.set_margin_start(8);
    sidebar_box.set_margin_end(8);

    let stack_sidebar = gtk::StackSidebar::new();
    stack_sidebar.set_stack(&view_stack);
    stack_sidebar.set_vexpand(true);
    sidebar_box.append(&stack_sidebar);

    let btn_content = adw::ButtonContent::builder()
        .icon_name("view-refresh-symbolic")
        .label("Reload Genubar Daemon")
        .build();
    let reload_btn = gtk::Button::builder()
        .child(&btn_content)
        .css_classes(["suggested-action"])
        .build();
    {
        let st = state.clone();
        reload_btn.connect_clicked(move |_| {
            st.flush_now(true);
        });
    }
    sidebar_box.append(&reload_btn);

    let split_paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .start_child(&sidebar_box)
        .end_child(&view_stack)
        .position(245)
        .shrink_start_child(false)
        .build();

    let root_vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.pack_end(&status_box);
    root_vbox.append(&header);
    root_vbox.append(&split_paned);

    toast_overlay.set_child(Some(&root_vbox));

    Ok(adw::ApplicationWindow::builder()
        .application(app)
        .title("Genubar Settings")
        .default_width(1080)
        .default_height(720)
        .content(&toast_overlay)
        .build())
}
