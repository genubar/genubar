use crate::components::rows::build_action_row;
use crate::services::cache_service::CacheService;
use crate::state::AppState;
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

pub fn build_cache_group(state: &AppState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Level-1 & Level-2 Menu Caches")
        .description("Manage cached root headers (l1_roots.json) and lazy AT-SPI2 submenus (atspi_menus.json)")
        .build();

    let l1_row = build_action_row(
        "Level-1 (L1) Root Menu Bar Cache",
        "Stores top-level tab skeletons per (app_id, window_role)",
    );
    let clear_l1_btn = gtk::Button::builder()
        .label("Clear L1 Cache")
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        clear_l1_btn.connect_clicked(move |_| match CacheService::clear_l1_cache() {
            Ok(()) => st.notify("Cleared L1 root menu cache (l1_roots.json)"),
            Err(e) => st.notify(&format!("Failed clearing L1 cache: {e}")),
        });
    }
    l1_row.add_suffix(&clear_l1_btn);
    group.add(&l1_row);

    let l2_row = build_action_row(
        "Level-2 (L2) AT-SPI2 Submenu Cache",
        "Stores deep submenu hierarchies for Firefox and Chromium",
    );
    let clear_l2_btn = gtk::Button::builder()
        .label("Clear L2 Cache")
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        clear_l2_btn.connect_clicked(move |_| match CacheService::clear_l2_atspi_cache() {
            Ok(()) => st.notify("Cleared L2 AT-SPI2 submenu cache (atspi_menus.json)"),
            Err(e) => st.notify(&format!("Failed clearing L2 cache: {e}")),
        });
    }
    l2_row.add_suffix(&clear_l2_btn);
    group.add(&l2_row);

    group
}
