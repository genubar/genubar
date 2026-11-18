mod package_row;
mod pagination;
mod sources_section;

use crate::components::controls::{
    build_scrolled_container, build_toolbar_dropdown, clear_box,
};
use crate::components::rows::build_action_row;
use crate::models::package_ui::category_filter_from_index;
use crate::state::AppState;
use genubar_packages::{RegistryPackageInstaller, UpdateService};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use package_row::{build_remote_package_row, build_search_result_row};
use pagination::{PAGE_SIZE, build_pagination_controls};
use sources_section::build_registry_sources_group;
use std::cell::RefCell;
use std::rc::Rc;

fn attach_pagination(
    group: &adw::PreferencesGroup,
    state: &AppState,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    page_idx: usize,
    total_items: usize,
) {
    if total_items > PAGE_SIZE {
        let st = state.clone();
        let rh = reload_handle.clone();
        group.add(&build_pagination_controls(
            page_idx,
            total_items,
            move |p| {
                st.registry_cache.borrow_mut().page = p;
                if let Some(ref cb) = *rh.borrow() {
                    cb();
                }
            },
        ));
    }
}

pub fn build_marketplace_view(state: &AppState) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let top_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top_bar.set_margin_top(12);
    top_bar.set_margin_bottom(6);
    top_bar.set_margin_start(16);
    top_bar.set_margin_end(16);

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search catalog & local packages..."));
    search_entry.set_hexpand(true);

    let kind_drop = build_toolbar_dropdown(
        &[
            "All Types",
            "Bundles",
            "Patches",
            "Profiles",
            "Styles",
            "Icon Packs",
        ],
        0,
    );

    let installed_btn = gtk::Button::builder().label("Installed").build();
    {
        let st = state.clone();
        installed_btn.connect_clicked(move |_| st.navigate_to("installed"));
    }

    let sync_btn = gtk::Button::builder()
        .label("Sync Catalog")
        .css_classes(["suggested-action"])
        .tooltip_text("Fetch packages from all configured registry.json sources")
        .build();

    top_bar.append(&search_entry);
    top_bar.append(&kind_drop);
    top_bar.append(&installed_btn);
    top_bar.append(&sync_btn);
    root_box.append(&top_bar);

    let (scrolled, container) = build_scrolled_container();
    root_box.append(&scrolled);

    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let rebuild_page: Rc<dyn Fn()> = {
        let state = state.clone();
        let container = container.clone();
        let search_entry = search_entry.clone();
        let kind_drop = kind_drop.clone();
        let reload_handle_inner = reload_handle.clone();

        Rc::new(move || {
            clear_box(&container);

            let page = adw::PreferencesPage::new();
            let query = search_entry.text().trim().to_string();
            let category = category_filter_from_index(kind_drop.selected());
            let cache_snap = state.registry_cache.borrow().clone();

            if !query.is_empty() || category.is_some() {
                let results_group = adw::PreferencesGroup::builder()
                    .title("Search Results")
                    .description(
                        "Clear search and reset filter to 'All Types' to return to Registry Sources",
                    )
                    .build();

                let items = RegistryPackageInstaller::search_packages(
                    &state.config.borrow(),
                    &state.patcher.borrow(),
                    &cache_snap.entries,
                    &query,
                    category,
                );

                if items.is_empty() {
                    results_group.add(&build_action_row(
                        "No matching packages found",
                        "Try another keyword or click Sync Catalog in the top bar",
                    ));
                } else {
                    let total = items.len();
                    let total_pages = total.div_ceil(PAGE_SIZE).max(1);
                    let page_idx = cache_snap.page.min(total_pages - 1);
                    let start = page_idx * PAGE_SIZE;
                    let end = (start + PAGE_SIZE).min(total);

                    for item in &items[start..end] {
                        results_group.add(&build_search_result_row(&state, item));
                    }
                    attach_pagination(&results_group, &state, &reload_handle_inner, page_idx, total);
                }

                page.add(&results_group);
                container.append(&page);
                return;
            }

            let remote_group = adw::PreferencesGroup::builder()
                .title("Remote Catalog Packages")
                .description(&cache_snap.summary)
                .build();

            let total_entries = cache_snap.entries.len();
            if total_entries == 0 {
                remote_group.add(&build_action_row(
                    "Catalog not synced yet",
                    "Click 'Sync Catalog' in the top bar to load packages from the sources below",
                ));
            } else {
                let total_pages = total_entries.div_ceil(PAGE_SIZE).max(1);
                let page_idx = cache_snap.page.min(total_pages - 1);
                let start = page_idx * PAGE_SIZE;
                let end = (start + PAGE_SIZE).min(total_entries);

                for pkg in &cache_snap.entries[start..end] {
                    remote_group.add(&build_remote_package_row(&state, pkg));
                }
                attach_pagination(
                    &remote_group,
                    &state,
                    &reload_handle_inner,
                    page_idx,
                    total_entries,
                );
            }
            page.add(&remote_group);
            page.add(&build_registry_sources_group(
                &state,
                &cache_snap.statuses,
                &reload_handle_inner,
            ));

            container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(rebuild_page.clone());
    state.register_tab_refresher("marketplace", rebuild_page.clone());
    rebuild_page();

    {
        let st = state.clone();
        let reb = rebuild_page.clone();
        search_entry.connect_search_changed(move |_| {
            st.registry_cache.borrow_mut().page = 0;
            reb();
        });
    }
    {
        let st = state.clone();
        let reb = rebuild_page.clone();
        kind_drop.connect_selected_notify(move |_| {
            st.registry_cache.borrow_mut().page = 0;
            reb();
        });
    }
    {
        let st = state.clone();
        let reb = rebuild_page;
        sync_btn.connect_clicked(move |btn| {
            let urls = st.config.borrow().settings.general.registry_urls.clone();
            btn.set_sensitive(false);
            btn.set_label("Syncing...");
            let btn_c = btn.clone();
            let st_in = st.clone();
            let reb_in = reb.clone();

            gtk::glib::spawn_future_local(async move {
                let (entries, statuses) = gtk::gio::spawn_blocking(move || {
                    UpdateService::fetch_remote_packages(&urls)
                })
                .await
                .unwrap_or_default();

                let ok_count = statuses.iter().filter(|s| s.error.is_none()).count();
                let err_count = statuses.iter().filter(|s| s.error.is_some()).count();

                let status_msg = if statuses.is_empty() {
                    String::from("No registry sources configured below")
                } else if entries.is_empty() && err_count > 0 {
                    let first_err = statuses
                        .iter()
                        .find_map(|s| s.error.as_deref())
                        .unwrap_or("Unknown error");
                    format!("No packages loaded ({err_count} source error(s): {first_err})")
                } else if err_count == 0 {
                    format!(
                        "Loaded {} package(s) from {ok_count} source(s)",
                        entries.len()
                    )
                } else {
                    format!(
                        "Loaded {} package(s) from {ok_count} source(s) ({err_count} source warning(s))",
                        entries.len()
                    )
                };

                {
                    let mut cache = st_in.registry_cache.borrow_mut();
                    cache.entries = entries;
                    cache.statuses = statuses;
                    cache.summary = status_msg.clone();
                    cache.page = 0;
                }
                st_in.notify(&status_msg);
                btn_c.set_sensitive(true);
                btn_c.set_label("Sync Catalog");
                reb_in();
            });
        });
    }

    root_box.upcast()
}
