use crate::components::controls::{build_icon_button, build_priority_arrows};
use crate::components::dialogs::show_info_modal;
use crate::components::rows::build_switch_row;
use crate::models::info_item::InfoField;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use crate::views::profiles_view::profile_editor::open_profile_dual_editor;
use adw::prelude::*;
use genubar_config::AppProfile;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn build_profile_row(
    state: &AppState,
    prof: &AppProfile,
    idx: usize,
    total_len: usize,
    match_desc: &[String],
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::ActionRow {
    let sub = format!(
        "by {} • id={} • match=[{}]",
        prof.author,
        prof.id,
        match_desc.join(", ")
    );
    let prof_id_for_switch = prof.id.clone();
    let row = build_switch_row(state, &prof.name, &sub, prof.enabled, move |st, active| {
        if let Err(e) = ConfigService::set_profile_enabled(
            &mut st.config.borrow_mut(),
            &prof_id_for_switch,
            active,
        ) {
            st.notify(&format!("Failed saving profile: {e}"));
        }
    });

    let st_up = state.clone();
    let rh_up = reload_handle.clone();
    let id_up = prof.id.clone();
    let st_down = state.clone();
    let rh_down = reload_handle.clone();
    let id_down = prof.id.clone();

    let (up_btn, down_btn) = build_priority_arrows(
        idx > 0,
        idx + 1 < total_len,
        move || match ConfigService::move_profile_by_id(&mut st_up.config.borrow_mut(), &id_up, -1)
        {
            Ok(true) => {
                st_up.save_and_reload();
                if let Some(ref cb) = *rh_up.borrow() {
                    cb();
                }
            }
            Ok(false) => {}
            Err(e) => st_up.notify(&format!("Reorder error: {e}")),
        },
        move || match ConfigService::move_profile_by_id(
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
        },
    );

    let info_btn = build_icon_button("dialog-information-symbolic", "Profile Details", &[]);
    {
        let p_info = prof.clone();
        let m_str = match_desc.join(", ");
        info_btn.connect_clicked(move |btn| {
            let fields = vec![
                InfoField::new("Profile ID", p_info.id.clone()),
                InfoField::new("Name", p_info.name.clone()),
                InfoField::new("Author", p_info.author.clone()),
                InfoField::new("Description", p_info.description.clone()),
                InfoField::new("Window Matchers", m_str.clone()),
                InfoField::new("Columns Defined", format!("{}", p_info.columns.len())),
            ];
            show_info_modal(btn, &format!("Profile: {}", p_info.name), &fields);
        });
    }

    let edit_btn = build_icon_button(
        "document-edit-symbolic",
        "Edit Profile (Visual / TOML)",
        &[],
    );
    {
        let st_ed = state.clone();
        let p_clone = prof.clone();
        let rh_ed = reload_handle.clone();
        edit_btn.connect_clicked(move |btn| {
            open_profile_dual_editor(btn, &st_ed, p_clone.clone(), &rh_ed);
        });
    }

    row.add_suffix(&up_btn);
    row.add_suffix(&down_btn);
    row.add_suffix(&info_btn);
    row.add_suffix(&edit_btn);
    row
}
