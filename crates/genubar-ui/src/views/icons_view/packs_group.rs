use crate::components::controls::{build_icon_button, build_priority_arrows, clear_box};
use crate::components::dialogs::{show_confirm_modal, show_info_modal};
use crate::components::rows::build_switch_row;
use crate::models::info_item::InfoField;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_icon_packs_group(
    state: &AppState,
    pack_query_state: &Rc<RefCell<String>>,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::PreferencesGroup {
    let packs_group = adw::PreferencesGroup::builder()
        .title("Installed Icon Packs (Ordered by Priority)")
        .description("Packs at the top evaluate first. Use ↑ / ↓ to reorder priority.")
        .build();

    let sec_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sec_bar.set_margin_top(8);
    sec_bar.set_margin_bottom(8);
    sec_bar.set_margin_start(12);
    sec_bar.set_margin_end(12);

    let sec_search = gtk::SearchEntry::builder()
        .placeholder_text("Search installed icon packs...")
        .text(pack_query_state.borrow().as_str())
        .hexpand(true)
        .build();

    let disable_all_btn = gtk::Button::builder()
        .label("Disable All")
        .valign(gtk::Align::Center)
        .build();

    sec_bar.append(&sec_search);
    sec_bar.append(&disable_all_btn);

    let sec_bar_row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&sec_bar)
        .build();
    packs_group.add(&sec_bar_row);

    let packs_list_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let packs_list_row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&packs_list_box)
        .build();

    let populate_packs: Rc<dyn Fn()> = {
        let state = state.clone();
        let packs_list_box = packs_list_box.clone();
        let sec_search = sec_search.clone();
        let reload_handle_inner = reload_handle.clone();

        Rc::new(move || {
            clear_box(&packs_list_box);
            let q = sec_search.text().trim().to_lowercase();
            let packs = state.config.borrow().icon_packs.clone();
            let total_packs = packs.len();

            for (idx, pack) in packs.into_iter().enumerate() {
                if !q.is_empty()
                    && !pack.name.to_lowercase().contains(&q)
                    && !pack.id.to_lowercase().contains(&q)
                    && !pack.author.to_lowercase().contains(&q)
                {
                    continue;
                }
                let author = if pack.author.is_empty() {
                    "local"
                } else {
                    &pack.author
                };
                let sub = format!(
                    "by {} • {} rules • {}",
                    author,
                    pack.rules.len(),
                    pack.description
                );
                let pack_id_sw = pack.id.clone();
                let row = build_switch_row(&state, &pack.name, &sub, pack.enabled, move |st, active| {
                    if let Err(e) = ConfigService::set_icon_pack_enabled(
                        &mut st.config.borrow_mut(),
                        &pack_id_sw,
                        active,
                    ) {
                        st.notify(&format!("Failed saving icon pack: {e}"));
                    }
                });

                let st_up = state.clone();
                let rh_up = reload_handle_inner.clone();
                let id_up = pack.id.clone();
                let st_down = state.clone();
                let rh_down = reload_handle_inner.clone();
                let id_down = pack.id.clone();

                let (up_btn, down_btn) = build_priority_arrows(
                    idx > 0,
                    idx + 1 < total_packs,
                    move || {
                        match ConfigService::move_icon_pack_by_id(
                            &mut st_up.config.borrow_mut(),
                            &id_up,
                            -1,
                        ) {
                            Ok(true) => {
                                st_up.save_and_reload();
                                if let Some(ref cb) = *rh_up.borrow() {
                                    cb();
                                }
                            }
                            Ok(false) => {}
                            Err(e) => st_up.notify(&format!("Reorder error: {e}")),
                        }
                    },
                    move || {
                        match ConfigService::move_icon_pack_by_id(
                            &mut st_down.config.borrow_mut(),
                            &id_down,
                            1,
                        ) {
                            Ok(true) => {
                                st_down.save_and_reload();
                                if let Some(ref cb) = *rh_down.borrow() {
                                    cb();
                                }
                            }
                            Ok(false) => {}
                            Err(e) => st_down.notify(&format!("Reorder error: {e}")),
                        }
                    },
                );

                let info_btn =
                    build_icon_button("dialog-information-symbolic", "Icon Pack Info", &[]);
                {
                    let p_clone = pack.clone();
                    info_btn.connect_clicked(move |btn| {
                        let fields = vec![
                            InfoField::new("Pack ID", p_clone.id.clone()),
                            InfoField::new("Name", p_clone.name.clone()),
                            InfoField::new("Author", p_clone.author.clone()),
                            InfoField::new("Description", p_clone.description.clone()),
                            InfoField::new("Rules Count", format!("{}", p_clone.rules.len())),
                            InfoField::new(
                                "Directory",
                                p_clone.pack_dir.to_string_lossy().to_string(),
                            ),
                        ];
                        show_info_modal(btn, &format!("Icon Pack: {}", p_clone.name), &fields);
                    });
                }

                row.add_suffix(&up_btn);
                row.add_suffix(&down_btn);
                row.add_suffix(&info_btn);
                packs_list_box.append(&row);
            }
        })
    };

    populate_packs();

    {
        let pop_p = populate_packs;
        let pq = pack_query_state.clone();
        sec_search.connect_search_changed(move |se| {
            *pq.borrow_mut() = se.text().to_string();
            pop_p();
        });
    }
    {
        let st = state.clone();
        let rh = reload_handle.clone();
        disable_all_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            let rh_in = rh.clone();
            show_confirm_modal(
                btn,
                "Disable All Icon Packs?",
                "Are you sure you want to disable all installed custom icon packs?",
                "Disable All",
                move || {
                    if let Err(e) =
                        ConfigService::disable_all_icon_packs(&mut st_in.config.borrow_mut())
                    {
                        st_in.notify(&format!("Failed disabling icon packs: {e}"));
                    }
                    st_in.save_and_reload();
                    if let Some(ref cb) = *rh_in.borrow() {
                        cb();
                    }
                },
            );
        });
    }

    packs_group.add(&packs_list_row);
    packs_group
}
