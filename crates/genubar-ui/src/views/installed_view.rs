use crate::components::controls::build_toolbar_dropdown;
use crate::components::rows::build_action_row;
use crate::models::package_ui::{
    installed_filter_from_index, package_kind_badge, package_kind_tab_id,
};
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use genubar_packages::{collect_all_installed_items, delete_installed_item};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Unified management view listing installed Bundles, Patches, Profiles, Styles, Icon Packs, and Scripts.
pub fn build_installed_view(state: &AppState) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let top_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top_bar.set_margin_top(12);
    top_bar.set_margin_bottom(6);
    top_bar.set_margin_start(16);
    top_bar.set_margin_end(16);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search all installed files by name, author, or ID...")
        .hexpand(true)
        .build();

    let kind_drop = build_toolbar_dropdown(
        &[
            "All Installed",
            "Bundles",
            "Patches",
            "Profiles",
            "Styles",
            "Icon Packs",
            "Python Scripts",
        ],
        0,
    );

    let refresh_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh installed files list")
        .build();

    top_bar.append(&search_entry);
    top_bar.append(&kind_drop);
    top_bar.append(&refresh_btn);
    root_box.append(&top_bar);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scrolled.set_child(Some(&container));
    root_box.append(&scrolled);

    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let populate: Rc<dyn Fn()> = {
        let state = state.clone();
        let container = container.clone();
        let search_entry = search_entry.clone();
        let kind_drop = kind_drop.clone();
        let reload_handle_inner = reload_handle.clone();

        Rc::new(move || {
            while let Some(c) = container.first_child() {
                container.remove(&c);
            }

            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::builder()
                .title("All Installed Packages & Configuration Files")
                .description("Manage, inspect, jump to the editor tab, or remove any installed component")
                .build();

            let q = search_entry.text().trim().to_lowercase();
            let filter_idx = kind_drop.selected();

            let all_items = collect_all_installed_items(
                &state.config.borrow(),
                &state.patcher.borrow(),
            );
            let selected_kind = installed_filter_from_index(filter_idx);

            let mut shown = 0usize;
            for item in all_items {
                if selected_kind.is_some_and(|k| item.kind != k) {
                    continue;
                }
                if !q.is_empty()
                    && !item.name.to_lowercase().contains(&q)
                    && !item.id.to_lowercase().contains(&q)
                    && !item.author.to_lowercase().contains(&q)
                {
                    continue;
                }

                let path_str = item
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| String::from("built-in"));

                let row = build_action_row(
                    &format!("[{}] {} (v{})", package_kind_badge(item.kind), item.name, item.version),
                    &format!("by {} • id={} • {}", item.author, item.id, path_str),
                );

                let jump_btn = gtk::Button::builder()
                    .label("Manage in Tab")
                    .valign(gtk::Align::Center)
                    .build();
                {
                    let st = state.clone();
                    let target = package_kind_tab_id(item.kind);
                    jump_btn.connect_clicked(move |_| st.navigate_to(target));
                }
                row.add_suffix(&jump_btn);

                let del_btn = gtk::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .tooltip_text(if item.can_delete {
                        "Delete Package"
                    } else {
                        "Built-in default package (cannot be deleted)"
                    })
                    .sensitive(item.can_delete)
                    .valign(gtk::Align::Center)
                    .css_classes(["destructive-action"])
                    .build();
                {
                    let st = state.clone();
                    let item_clone = item.clone();
                    let rh = reload_handle_inner.clone();
                    del_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        let btn_c = btn.clone();
                        let st_in = st.clone();
                        let item_in = item_clone.clone();
                        let deleted_name = item_clone.name.clone();
                        let rh_in = rh.clone();
                        let mut cfg = st.config.borrow().clone();
                        let mut patcher = st.patcher.borrow().clone();
                        gtk::glib::spawn_future_local(async move {
                            let res = gtk::gio::spawn_blocking(move || {
                                delete_installed_item(&mut cfg, &mut patcher, &item_in)
                                    .map(|()| (cfg, patcher))
                            })
                            .await;
                            btn_c.set_sensitive(true);
                            match res {
                                Ok(Ok((fresh_cfg, fresh_patcher))) => {
                                    *st_in.config.borrow_mut() = fresh_cfg;
                                    *st_in.patcher.borrow_mut() = fresh_patcher;
                                    st_in.notify(&format!("Deleted '{deleted_name}'"));
                                    st_in.save_and_reload();
                                    if let Some(ref cb) = *rh_in.borrow() {
                                        cb();
                                    }
                                }
                                Ok(Err(e)) => st_in.notify(&e.to_string()),
                                Err(e) => st_in.notify(&format!("Background delete error: {e:?}")),
                            }
                        });
                    });
                }
                row.add_suffix(&del_btn);
                group.add(&row);
                shown += 1;
            }

            if shown == 0 {
                group.add(&build_action_row(
                    "No matching installed packages",
                    "Try clearing the search filter",
                ));
            }

            page.add(&group);
            container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(populate.clone());
    state.register_tab_refresher("installed", populate.clone());
    populate();

    {
        let pop = populate.clone();
        search_entry.connect_search_changed(move |_| pop());
    }
    {
        let pop = populate.clone();
        kind_drop.connect_selected_notify(move |_| pop());
    }
    {
        let st = state.clone();
        let pop = populate;
        refresh_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            let btn_c = btn.clone();
            let st_in = st.clone();
            let pop_in = pop.clone();
            gtk::glib::spawn_future_local(async move {
                let res =
                    gtk::gio::spawn_blocking(ConfigService::reload_config_and_patcher).await;
                btn_c.set_sensitive(true);
                match res {
                    Ok(Ok((cfg, pat))) => {
                        *st_in.config.borrow_mut() = cfg;
                        *st_in.patcher.borrow_mut() = pat;
                        pop_in();
                    }
                    Ok(Err(e)) => st_in.notify(&format!("Reload error: {e}")),
                    Err(e) => st_in.notify(&format!("Task error: {e:?}")),
                }
            });
        });
    }

    root_box.upcast()
}