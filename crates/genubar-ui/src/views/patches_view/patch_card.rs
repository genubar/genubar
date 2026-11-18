use crate::components::controls::{
    build_caption_label, build_compat_warning_label, build_icon_button, build_popover_menu_button,
};
use crate::components::dialogs::show_info_modal;
use crate::components::rows::build_action_row;
use crate::error::UiError;
use crate::models::info_item::InfoField;
use crate::services::patch_service::{PatchOpOutcome, PatchService};
use crate::state::AppState;
use genubar_patcher::{ConflictStrategy, PackageStatus, PatchCardView, PatchManager};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn spawn_patch_card_op<F>(
    state: &AppState,
    trigger: &impl IsA<gtk::Widget>,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    rebuild_all: bool,
    op: F,
) where
    F: FnOnce(PatchManager) -> Result<PatchOpOutcome, UiError> + Send + 'static,
{
    let patcher = state.patcher.borrow().clone();
    trigger.set_sensitive(false);

    let st = state.clone();
    let rh = reload_handle.clone();
    let btn = trigger.clone();
    gtk::glib::spawn_future_local(async move {
        let res = gtk::gio::spawn_blocking(move || op(patcher)).await;
        btn.set_sensitive(true);
        match res {
            Ok(Ok(outcome)) => {
                *st.patcher.borrow_mut() = outcome.patcher;
                st.notify(&outcome.message);
                if outcome.needs_reload {
                    st.save_and_reload();
                }
                if rebuild_all {
                    st.rebuild_all_views();
                } else if let Some(ref cb) = *rh.borrow() {
                    cb();
                }
            }
            Ok(Err(e)) => {
                st.notify(&format!("Patch rolled back: {e}"));
                if let Some(ref cb) = *rh.borrow() {
                    cb();
                }
            }
            Err(e) => {
                let task_err = UiError::Task(format!("{e:?}"));
                st.notify(&task_err.to_string());
            }
        }
    });
}

fn build_patch_info_button(card: &PatchCardView) -> gtk::Button {
    let info_btn = build_icon_button("dialog-information-symbolic", "View Patch Details", &[]);
    let card_clone = card.clone();
    info_btn.connect_clicked(move |btn| {
        let meta = &card_clone.manifest.meta;
        let det = &card_clone.manifest.detection;
        let mut fields = vec![
            InfoField::new("Patch ID", meta.id.clone()),
            InfoField::new("Name", meta.name.clone()),
            InfoField::new("Version", meta.version.clone()),
            InfoField::new("Author", meta.author.clone()),
            InfoField::new(
                "Min App Version",
                meta.min_app_version
                    .clone()
                    .unwrap_or_else(|| String::from("Any")),
            ),
            InfoField::new("Tags", meta.tags.join(", ")),
            InfoField::new("Description", meta.description.clone()),
            InfoField::new(
                "Restart Hint",
                meta.restart_hint
                    .clone()
                    .unwrap_or_else(|| String::from("None")),
            ),
            InfoField::new(
                "Target Detection",
                format!(
                    "bins={:?}, desktop={:?}, flatpak={:?}",
                    det.binaries, det.desktop_files, det.flatpak_ids
                ),
            ),
            InfoField::new(
                "Total Patch Steps",
                format!("{} step(s)", card_clone.manifest.steps.len()),
            ),
        ];
        if let Some(ref reason) = card_clone.incompatibility_reason {
            fields.push(InfoField::new("Compatibility", reason.clone()));
        }
        show_info_modal(btn, &format!("Patch Info: {}", meta.name), &fields);
    });
    info_btn
}

fn build_patch_overflow_menu(
    state: &AppState,
    card: &PatchCardView,
    patch_file: PathBuf,
    is_applied: bool,
    can_delete: bool,
    strategy_state: &Rc<RefCell<ConflictStrategy>>,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> gtk::MenuButton {
    let patch_id = card.manifest.meta.id.clone();
    let (more_btn, pop, pop_box) = build_popover_menu_button("More Options", 8);

    if !card.is_compatible && !is_applied {
        let reason_text = card
            .incompatibility_reason
            .as_deref()
            .unwrap_or("Requires a newer Genubar release");
        let warn_lbl = build_compat_warning_label(reason_text);
        let force_btn = gtk::Button::builder()
            .label("Apply Anyway (Skip Unsupported Steps)")
            .css_classes(["flat"])
            .build();

        let st = state.clone();
        let strat_c = strategy_state.clone();
        let rh = reload_handle.clone();
        let pid = patch_id.clone();
        let pop_c = pop.clone();
        let mb = more_btn.clone();
        force_btn.connect_clicked(move |_| {
            pop_c.popdown();
            let policy = st.config.borrow().settings.general.patch_error_policy;
            let strat = *strat_c.borrow();
            let pid_c = pid.clone();
            spawn_patch_card_op(&st, &mb, &rh, false, move |patcher| {
                PatchService::apply_owned(patcher, pid_c, strat, policy, true)
            });
        });

        pop_box.append(&warn_lbl);
        pop_box.append(&force_btn);
    }

    if can_delete {
        if !card.is_compatible && !is_applied {
            pop_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        let del_btn = gtk::Button::builder()
            .label("Delete Patch File")
            .css_classes(["flat", "destructive-action"])
            .build();

        let st = state.clone();
        let pid = patch_id;
        let pfile = patch_file;
        let pop_c = pop;
        let rh = reload_handle.clone();
        let mb = more_btn.clone();
        del_btn.connect_clicked(move |_| {
            pop_c.popdown();
            let pid_c = pid.clone();
            let pf_c = pfile.clone();
            spawn_patch_card_op(&st, &mb, &rh, true, move |patcher| {
                PatchService::delete_owned(patcher, pid_c, pf_c)
            });
        });

        pop_box.append(&del_btn);
    }

    more_btn
}

pub fn build_patch_card_row(
    state: &AppState,
    card: &PatchCardView,
    strategy_state: &Rc<RefCell<ConflictStrategy>>,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::ActionRow {
    let m = &card.manifest.meta;
    let is_applied = card.status == PackageStatus::Applied;

    let subtitle = format!("v{} • by {} — {}", m.version, m.author, m.description);
    let row = build_action_row(&m.name, &subtitle);
    row.set_title_lines(1);
    row.set_subtitle_lines(2);

    let patch_file = PatchService::patch_file_path(&state.config.borrow(), &m.id);
    let can_delete = patch_file.is_file();

    if !card.is_compatible && !is_applied {
        let reason_text = card
            .incompatibility_reason
            .as_deref()
            .unwrap_or("Update Required");
        row.add_suffix(&build_caption_label(reason_text, Some("dim-label")));
    }

    row.add_suffix(&build_patch_info_button(card));

    let patch_id = m.id.clone();

    if card.is_compatible || is_applied {
        let action_btn = gtk::Button::builder()
            .label(if is_applied { "Revert" } else { "Apply Patch" })
            .valign(gtk::Align::Center)
            .css_classes(if is_applied {
                vec!["destructive-action"]
            } else {
                vec!["suggested-action"]
            })
            .build();

        let st = state.clone();
        let strat_c = strategy_state.clone();
        let rh = reload_handle.clone();

        action_btn.connect_clicked(move |btn| {
            let policy = st.config.borrow().settings.general.patch_error_policy;
            let strat = *strat_c.borrow();
            let pid_c = patch_id.clone();
            spawn_patch_card_op(&st, btn, &rh, false, move |patcher| {
                if is_applied {
                    PatchService::revert_owned(patcher, pid_c)
                } else {
                    PatchService::apply_owned(patcher, pid_c, strat, policy, false)
                }
            });
        });

        row.add_suffix(&action_btn);
    }

    if (!card.is_compatible && !is_applied) || can_delete {
        row.add_suffix(&build_patch_overflow_menu(
            state,
            card,
            patch_file,
            is_applied,
            can_delete,
            strategy_state,
            reload_handle,
        ));
    }

    row
}
