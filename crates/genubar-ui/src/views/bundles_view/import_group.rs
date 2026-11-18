use crate::components::rows::build_two_row_entry;
use crate::services::bundle_service::BundleService;
use crate::state::AppState;
use genubar_platform::expand_tilde;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub(super) fn build_import_bundle_group(state: &AppState) -> adw::PreferencesGroup {
    let import_group = adw::PreferencesGroup::builder()
        .title("Install Bundle from Local Folder")
        .description(
            "Import a directory containing bundle.toml and component folders (patches/, profiles/, styles/, icons/)",
        )
        .build();

    let (imp_row, folder_entry, inst_btn) = build_two_row_entry(
        "Bundle Folder Path",
        "Enter a directory path with bundle.toml",
        "",
        "~/Downloads/macos_sequoia_bundle",
        "Install Bundle",
    );
    {
        let st = state.clone();
        let fe = folder_entry;
        inst_btn.connect_clicked(move |btn| {
            let expanded = expand_tilde(&fe.text());
            let base_dir = st.config.borrow().base_dir().to_path_buf();
            btn.set_sensitive(false);
            btn.set_label("Installing...");

            let btn_c = btn.clone();
            let st_in = st.clone();
            gtk::glib::spawn_future_local(async move {
                let res = gtk::gio::spawn_blocking(move || {
                    BundleService::install_from_dir_owned(base_dir, expanded)
                })
                .await;

                btn_c.set_sensitive(true);
                btn_c.set_label("Install Bundle");
                match res {
                    Ok(Ok(outcome)) => {
                        *st_in.config.borrow_mut() = outcome.config;
                        *st_in.patcher.borrow_mut() = outcome.patcher;
                        st_in.notify(&outcome.message);
                        st_in.save_and_reload();
                        st_in.rebuild_all_views();
                    }
                    Ok(Err(e)) => st_in.notify(&format!("Bundle error: {e}")),
                    Err(e) => st_in.notify(&format!("Background task error: {e:?}")),
                }
            });
        });
    }
    import_group.add(&imp_row);
    import_group
}
