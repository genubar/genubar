mod profile_editor;
mod profile_row;
mod snapshots_group;

use crate::components::controls::{
    build_scrolled_container, build_section_top_bar, clear_box, spawn_config_reload,
};
use crate::components::dialogs::{show_confirm_modal, show_text_prompt_modal};
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use adw::prelude::*;
use genubar_platform::expand_tilde;
use gtk4 as gtk;
use libadwaita as adw;
use profile_editor::open_profile_dual_editor;
use profile_row::build_profile_row;
use snapshots_group::build_snapshots_group;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_profiles_view(state: &AppState) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let query_state = Rc::new(RefCell::new(String::new()));

    let top_bar = build_section_top_bar(
        state,
        "Open Profiles Folder",
        &reload_handle,
        |st| st.config.borrow().base_dir().join("profiles"),
        {
            let rh = reload_handle.clone();
            move |st| spawn_config_reload(st, &rh)
        },
        |st| {
            if let Err(e) = st.config.borrow_mut().reset_profiles_to_defaults() {
                st.notify(&format!("Reset error: {e}"));
            }
        },
    );
    root_box.append(&top_bar);

    let (scrolled, container) = build_scrolled_container();
    root_box.append(&scrolled);

    let rebuild_profiles: Rc<dyn Fn()> = {
        let state = state.clone();
        let container = container.clone();
        let reload_handle_inner = reload_handle.clone();

        Rc::new(move || {
            clear_box(&container);

            let page = adw::PreferencesPage::new();
            let prof_group = adw::PreferencesGroup::builder()
                .title("Application & Global Plugin Profiles")
                .description(
                    "Higher priority profiles match first. Drag order with ▲▼ or click Edit to customize menus and scripts.",
                )
                .build();

            let sec_vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
            sec_vbox.set_margin_top(10);
            sec_vbox.set_margin_bottom(10);
            sec_vbox.set_margin_start(12);
            sec_vbox.set_margin_end(12);

            let sec_search = gtk::SearchEntry::builder()
                .placeholder_text("Search profiles by name, ID, author, or app matcher...")
                .text(query_state.borrow().as_str())
                .hexpand(true)
                .build();
            sec_vbox.append(&sec_search);

            let btn_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let new_btn = gtk::Button::builder()
                .label("+ Create New Profile")
                .css_classes(["suggested-action"])
                .build();
            let import_btn = gtk::Button::builder().label("Import Profile").build();
            let spacer = gtk::Box::builder().hexpand(true).build();
            let disable_all_btn = gtk::Button::builder().label("Disable All Profiles").build();

            btn_bar.append(&new_btn);
            btn_bar.append(&import_btn);
            btn_bar.append(&spacer);
            btn_bar.append(&disable_all_btn);
            sec_vbox.append(&btn_bar);

            let controls_row = gtk::ListBoxRow::builder()
                .activatable(false)
                .selectable(false)
                .child(&sec_vbox)
                .build();
            prof_group.add(&controls_row);

            let list_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let list_box_row = gtk::ListBoxRow::builder()
                .activatable(false)
                .selectable(false)
                .child(&list_box)
                .build();

            let populate_items: Rc<dyn Fn()> = {
                let state = state.clone();
                let list_box = list_box.clone();
                let sec_search = sec_search.clone();
                let reload_handle_inner = reload_handle_inner.clone();

                Rc::new(move || {
                    clear_box(&list_box);
                    let query = sec_search.text().trim().to_lowercase();
                    let profiles = state.config.borrow().profiles.clone();
                    let total_len = profiles.len();

                    for (idx, prof) in profiles.into_iter().enumerate() {
                        let match_desc: Vec<String> = prof
                            .matchers
                            .iter()
                            .map(|m| format!("{:?}: '{}'", m.field, m.contains))
                            .collect();

                        if !query.is_empty()
                            && !prof.name.to_lowercase().contains(&query)
                            && !prof.id.to_lowercase().contains(&query)
                            && !prof.author.to_lowercase().contains(&query)
                            && !match_desc.join(" ").to_lowercase().contains(&query)
                        {
                            continue;
                        }

                        list_box.append(&build_profile_row(
                            &state,
                            &prof,
                            idx,
                            total_len,
                            &match_desc,
                            &reload_handle_inner,
                        ));
                    }
                })
            };

            populate_items();

            {
                let pop = populate_items;
                let qs = query_state.clone();
                sec_search.connect_search_changed(move |se| {
                    *qs.borrow_mut() = se.text().to_string();
                    pop();
                });
            }
            {
                let st = state.clone();
                let rh = reload_handle_inner.clone();
                new_btn.connect_clicked(move |btn| {
                    match ConfigService::create_default_profile(&mut st.config.borrow_mut()) {
                        Ok(created) => {
                            if let Some(ref cb) = *rh.borrow() {
                                cb();
                            }
                            open_profile_dual_editor(btn, &st, created, &rh);
                        }
                        Err(e) => st.notify(&format!("Failed creating profile: {e}")),
                    }
                });
            }
            {
                let st = state.clone();
                let rh = reload_handle_inner.clone();
                import_btn.connect_clicked(move |btn| {
                    let st_in = st.clone();
                    let rh_in = rh.clone();
                    show_text_prompt_modal(
                        btn,
                        "Import App Profile (.toml)",
                        "",
                        "~/Downloads/my_profile.toml",
                        "Import into ~/.config/genubar/profiles/",
                        move |raw_path| {
                            let expanded = expand_tilde(&raw_path);
                            let res = ConfigService::import_profile_from_path(
                                &mut st_in.config.borrow_mut(),
                                &expanded,
                            );
                            match res {
                                Ok(name) => {
                                    st_in.notify(&format!("Imported profile '{name}'"));
                                    st_in.save_and_reload();
                                    if let Some(ref cb) = *rh_in.borrow() {
                                        cb();
                                    }
                                    true
                                }
                                Err(e) => {
                                    st_in.notify(&format!("Import failed: {e}"));
                                    false
                                }
                            }
                        },
                    );
                });
            }
            {
                let st = state.clone();
                let rh = reload_handle_inner.clone();
                disable_all_btn.connect_clicked(move |btn| {
                    let st_in = st.clone();
                    let rh_in = rh.clone();
                    show_confirm_modal(
                        btn,
                        "Disable All Profiles?",
                        "Are you sure you want to disable all application and plugin profiles?",
                        "Disable All",
                        move || {
                            if let Err(e) =
                                ConfigService::disable_all_profiles(&mut st_in.config.borrow_mut())
                            {
                                st_in.notify(&format!("Error disabling profiles: {e}"));
                            }
                            st_in.save_and_reload();
                            if let Some(ref cb) = *rh_in.borrow() {
                                cb();
                            }
                        },
                    );
                });
            }

            prof_group.add(&list_box_row);
            page.add(&prof_group);
            page.add(&build_snapshots_group(&state, &reload_handle_inner));
            container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(rebuild_profiles.clone());
    state.register_tab_refresher("profiles", rebuild_profiles.clone());
    rebuild_profiles();

    root_box.upcast()
}
