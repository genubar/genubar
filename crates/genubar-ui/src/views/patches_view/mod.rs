mod patch_card;
mod policy_group;

use crate::components::controls::{
    build_scrolled_container, build_section_top_bar, clear_box,
};
use crate::components::dialogs::{show_confirm_modal, show_text_prompt_modal};
use crate::components::rows::build_expander_row;
use crate::services::patch_service::{PatchOpOutcome, PatchService};
use crate::state::AppState;
use genubar_patcher::{ConflictStrategy, PackageStatus};
use genubar_platform::expand_tilde;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use patch_card::build_patch_card_row;
use policy_group::build_policy_group;
use std::cell::RefCell;
use std::rc::Rc;

fn finish_patch_list_op(
    state: &AppState,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    outcome: PatchOpOutcome,
) {
    *state.patcher.borrow_mut() = outcome.patcher;
    state.notify(&outcome.message);
    if outcome.needs_reload {
        state.save_and_reload();
    }
    if let Some(ref cb) = *reload_handle.borrow() {
        cb();
    }
}

pub fn build_patches_view(state: &AppState, initial_filter: Option<String>) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let strategy_state = Rc::new(RefCell::new(ConflictStrategy::Overwrite));
    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let query_state = Rc::new(RefCell::new(initial_filter.unwrap_or_default()));

    let top_bar = build_section_top_bar(
        state,
        "Open Patches Folder",
        &reload_handle,
        |st| st.config.borrow().base_dir().join("patches"),
        {
            let rh = reload_handle.clone();
            move |st| {
                let st_in = st.clone();
                let rh_in = rh.clone();
                gtk::glib::spawn_future_local(async move {
                    match gtk::gio::spawn_blocking(PatchService::reload_patcher_owned).await {
                        Ok(Ok(reloaded)) => {
                            *st_in.patcher.borrow_mut() = reloaded;
                            if let Some(ref cb) = *rh_in.borrow() {
                                cb();
                            }
                        }
                        Ok(Err(e)) => st_in.notify(&format!("Reload error: {e}")),
                        Err(e) => st_in.notify(&format!("Task error: {e:?}")),
                    }
                });
            }
        },
        {
            let rh = reload_handle.clone();
            move |st| {
                let patcher = st.patcher.borrow().clone();
                let st_in = st.clone();
                let rh_in = rh.clone();
                gtk::glib::spawn_future_local(async move {
                    if let Ok(outcome) = gtk::gio::spawn_blocking(move || {
                        PatchService::revert_all_owned(patcher)
                    })
                    .await
                    {
                        finish_patch_list_op(&st_in, &rh_in, outcome);
                    }
                });
            }
        },
    );
    root_box.append(&top_bar);

    let (scrolled, list_container) = build_scrolled_container();
    root_box.append(&scrolled);

    let populate_list: Rc<dyn Fn()> = {
        let state = state.clone();
        let list_container = list_container.clone();
        let reload_handle_inner = reload_handle.clone();

        Rc::new(move || {
            clear_box(&list_container);

            let page = adw::PreferencesPage::new();
            page.add(&build_policy_group(&state, &strategy_state));

            let sections_group = adw::PreferencesGroup::builder()
                .title("Patch Catalog")
                .description("Search, import, apply, or revert application patches")
                .build();

            let sec_vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
            sec_vbox.set_margin_top(10);
            sec_vbox.set_margin_bottom(10);
            sec_vbox.set_margin_start(12);
            sec_vbox.set_margin_end(12);

            let sec_search = gtk::SearchEntry::builder()
                .placeholder_text("Search patches by name, author, app, or #tag...")
                .text(query_state.borrow().as_str())
                .hexpand(true)
                .build();
            sec_vbox.append(&sec_search);

            let btn_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let import_btn = gtk::Button::builder()
                .label("Import Patch")
                .css_classes(["suggested-action"])
                .build();
            let spacer = gtk::Box::builder().hexpand(true).build();
            let disable_all_btn = gtk::Button::builder().label("Revert All Applied").build();

            btn_bar.append(&import_btn);
            btn_bar.append(&spacer);
            btn_bar.append(&disable_all_btn);
            sec_vbox.append(&btn_bar);

            let sec_controls_row = gtk::ListBoxRow::builder()
                .activatable(false)
                .selectable(false)
                .child(&sec_vbox)
                .build();
            sections_group.add(&sec_controls_row);

            let accordions_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let accordions_row = gtk::ListBoxRow::builder()
                .activatable(false)
                .selectable(false)
                .child(&accordions_box)
                .build();

            let populate_accordions: Rc<dyn Fn()> = {
                let state = state.clone();
                let accordions_box = accordions_box.clone();
                let sec_search = sec_search.clone();
                let strategy_state = strategy_state.clone();
                let reload_handle_inner = reload_handle_inner.clone();

                Rc::new(move || {
                    clear_box(&accordions_box);
                    let query = sec_search.text().trim().to_lowercase();
                    let cards = state.patcher.borrow().list_patch_cards();
                    let filtered: Vec<_> = cards
                        .into_iter()
                        .filter(|c| {
                            if query.is_empty() {
                                return true;
                            }
                            c.manifest.meta.name.to_lowercase().contains(&query)
                                || c.manifest.meta.author.to_lowercase().contains(&query)
                                || c.manifest.meta.id.to_lowercase().contains(&query)
                                || c.manifest.meta.description.to_lowercase().contains(&query)
                                || c.manifest
                                    .meta
                                    .tags
                                    .iter()
                                    .any(|t| t.to_lowercase().contains(&query))
                        })
                        .collect();

                    let applied_sec = build_expander_row(
                        "Applied Patches",
                        "Active modifications currently tracked in patch_ledger.json",
                        true,
                    );
                    let rec_sec = build_expander_row(
                        "Recommended for Your System",
                        "Patches matching applications detected on this computer",
                        true,
                    );
                    let all_sec = build_expander_row(
                        "All Available Patches",
                        "Saved local, built-in, and imported patches",
                        true,
                    );

                    for card in filtered {
                        let row = build_patch_card_row(
                            &state,
                            &card,
                            &strategy_state,
                            &reload_handle_inner,
                        );
                        if card.status == PackageStatus::Applied {
                            applied_sec.add_row(&row);
                        } else if card.is_recommended {
                            rec_sec.add_row(&row);
                        } else {
                            all_sec.add_row(&row);
                        }
                    }

                    accordions_box.append(&applied_sec);
                    accordions_box.append(&rec_sec);
                    accordions_box.append(&all_sec);
                })
            };

            populate_accordions();

            {
                let pop_a = populate_accordions;
                let qs = query_state.clone();
                sec_search.connect_search_changed(move |se| {
                    *qs.borrow_mut() = se.text().to_string();
                    pop_a();
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
                        "Import Local Patch (.toml)",
                        "",
                        "~/Downloads/my_patch.toml",
                        "Import into ~/.config/genubar/patches/",
                        move |raw| {
                            let expanded = expand_tilde(&raw);
                            let mut patcher = st_in.patcher.borrow().clone();
                            match patcher.import_local_patch(&expanded) {
                                Ok(ids) => {
                                    *st_in.patcher.borrow_mut() = patcher;
                                    st_in.notify(&format!("Imported patch(es): {}", ids.join(", ")));
                                    if let Some(ref cb) = *rh_in.borrow() {
                                        cb();
                                    }
                                    true
                                }
                                Err(e) => {
                                    st_in.notify(&format!("Import error: {e}"));
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
                        "Revert All Applied Patches?",
                        "Are you sure you want to revert all currently applied patches and restore original application settings?",
                        "Revert All",
                        move || {
                            let patcher = st_in.patcher.borrow().clone();
                            let st_bg = st_in.clone();
                            let rh_bg = rh_in.clone();
                            gtk::glib::spawn_future_local(async move {
                                if let Ok(outcome) = gtk::gio::spawn_blocking(move || {
                                    PatchService::revert_all_owned(patcher)
                                })
                                .await
                                {
                                    finish_patch_list_op(&st_bg, &rh_bg, outcome);
                                }
                            });
                        },
                    );
                });
            }

            sections_group.add(&accordions_row);
            page.add(&sections_group);
            list_container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(populate_list.clone());
    state.register_tab_refresher("patches", populate_list.clone());
    populate_list();

    root_box.upcast()
}
