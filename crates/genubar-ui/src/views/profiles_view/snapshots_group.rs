use crate::components::controls::build_icon_button;
use crate::components::dialogs::show_confirm_modal;
use crate::components::rows::build_action_row;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_snapshots_group(
    state: &AppState,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::PreferencesGroup {
    let snap_group = adw::PreferencesGroup::builder()
        .title("Captured Menu Snapshots")
        .description(
            "Tip: By default, you can capture a menu snapshot of any focused application directly from the top panel via {App Name} ▸ Capture Menu Snapshot.",
        )
        .build();

    let cleanup_row = build_action_row(
        "Snapshot Storage (~/.config/genubar/snapshots/)",
        "Delete snapshots that do not have a corresponding profile yet",
    );
    let del_unused_btn = gtk::Button::builder()
        .label("Delete All Unused")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    {
        let st = state.clone();
        let rh = reload_handle.clone();
        del_unused_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            let rh_in = rh.clone();
            show_confirm_modal(
                btn,
                "Delete All Unused Snapshots?",
                "This will permanently delete all captured menu snapshots that do not have a matching profile.",
                "Delete Unused",
                move || {
                    let count = ConfigService::delete_unused_snapshots(&st_in.config.borrow());
                    st_in.notify(&format!("Deleted {count} unused snapshot(s)"));
                    if let Some(ref cb) = *rh_in.borrow() {
                        cb();
                    }
                },
            );
        });
    }
    cleanup_row.add_suffix(&del_unused_btn);
    snap_group.add(&cleanup_row);

    let snapshots = ConfigService::list_snapshot_files(&state.config.borrow());
    for (fname, path, is_used) in snapshots {
        let status_badge = if is_used { "Profile exists" } else { "Unused" };
        let s_row = build_action_row(
            &fname,
            &format!("[{status_badge}] • {}", path.to_string_lossy()),
        );

        let gen_btn = gtk::Button::builder()
            .label("Generate Profile")
            .valign(gtk::Align::Center)
            .build();
        let del_snap_btn = build_icon_button(
            "user-trash-symbolic",
            "Delete Snapshot",
            &["destructive-action"],
        );

        {
            let st_gen = state.clone();
            let rh_gen = reload_handle.clone();
            let p_gen = path.clone();
            gen_btn.connect_clicked(move |_| {
                let res = ConfigService::create_profile_from_snapshot_and_reload(
                    &st_gen.config.borrow(),
                    &p_gen,
                );
                match res {
                    Ok((out, reloaded)) => {
                        *st_gen.config.borrow_mut() = reloaded;
                        st_gen.notify(&format!("Created profile: {}", out.display()));
                        st_gen.save_and_reload();
                        if let Some(ref cb) = *rh_gen.borrow() {
                            cb();
                        }
                    }
                    Err(e) => st_gen.notify(&format!("Failed creating profile: {e}")),
                }
            });
        }
        {
            let st_del = state.clone();
            let rh_del = reload_handle.clone();
            del_snap_btn.connect_clicked(move |_| {
                if ConfigService::delete_snapshot(&path) {
                    st_del.notify("Deleted snapshot file");
                    if let Some(ref cb) = *rh_del.borrow() {
                        cb();
                    }
                }
            });
        }

        s_row.add_suffix(&gen_btn);
        s_row.add_suffix(&del_snap_btn);
        snap_group.add(&s_row);
    }

    snap_group
}
