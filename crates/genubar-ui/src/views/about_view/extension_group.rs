use crate::components::rows::build_action_row;
use genubar_packages::UpdateService;
use crate::state::AppState;
use crate::utils::system::open_external;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use genubar_core::{EXPECTED_EXTENSION_VERSION, EXTENSION_UUID};
use std::cell::RefCell;
use std::rc::Rc;

pub struct ExtensionWidgets {
    pub group: adw::PreferencesGroup,
    pub ext_row: adw::ActionRow,
    pub update_btn: gtk::Button,
    pub release_btn: gtk::Button,
    pub raw_base_state: Rc<RefCell<Option<String>>>,
    pub release_url_state: Rc<RefCell<String>>,
}

pub fn build_extension_group(state: &AppState) -> ExtensionWidgets {
    let ext_group = adw::PreferencesGroup::builder()
        .title(format!("GNOME Shell Extension ({EXTENSION_UUID})"))
        .description(
            "Checks two-way compatibility with Daemon and allows installing or updating the GNOME extension",
        )
        .build();

    let ext_row = build_action_row(
        &format!("Checking GNOME Shell Extension... (Expected: v{EXPECTED_EXTENSION_VERSION})"),
        "Inspecting local extension metadata",
    );

    let ext_sync_btn = gtk::Button::builder()
        .label("Install / Sync Local")
        .valign(gtk::Align::Center)
        .tooltip_text(format!(
            "Install or synchronize {EXTENSION_UUID} from local workspace / embedded files"
        ))
        .build();

    {
        let ext_row_init = ext_row.clone();
        let ext_sync_init = ext_sync_btn.clone();
        gtk::glib::spawn_future_local(async move {
            if let Ok(ext_info) =
                gtk::gio::spawn_blocking(UpdateService::inspect_installed_extension).await
            {
                let ext_title = if ext_info.installed {
                    format!(
                        "Extension Version: v{} (Expected: v{EXPECTED_EXTENSION_VERSION})",
                        ext_info.version
                    )
                } else {
                    format!("Extension Not Installed (Expected: v{EXPECTED_EXTENSION_VERSION})")
                };
                ext_row_init.set_title(&ext_title);
                ext_row_init.set_subtitle(&ext_info.status_summary);
                if ext_info.installed {
                    ext_row_init.set_tooltip_text(Some(&format!(
                        "Installed v{} • Min Daemon v{} • Tested Daemon v{} • IPC v{}",
                        ext_info.version,
                        ext_info.min_daemon_version,
                        ext_info.tested_daemon_version,
                        ext_info.ipc_protocol_version,
                    )));
                    ext_sync_init.set_label("Install / Sync Local");
                    ext_sync_init.remove_css_class("suggested-action");
                } else {
                    ext_sync_init.set_label("Install Extension");
                    ext_sync_init.add_css_class("suggested-action");
                }
            }
        });
    }

    let ext_update_btn = gtk::Button::builder()
        .label("Update Extension")
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .visible(false)
        .build();

    let ext_release_btn = gtk::Button::builder()
        .label("Extension Release ↗")
        .valign(gtk::Align::Center)
        .visible(false)
        .build();

    let ext_raw_base_state: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let ext_release_url_state = Rc::new(RefCell::new(String::from(
        "https://github.com/genubar/genubar-gnome-extension/releases",
    )));

    {
        let st = state.clone();
        let ext_row_c = ext_row.clone();
        ext_sync_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Installing...");
            let btn_c = btn.clone();
            let st_in = st.clone();
            let ext_row_in = ext_row_c.clone();

            gtk::glib::spawn_future_local(async move {
                let res = gtk::gio::spawn_blocking(UpdateService::install_local_or_embedded_extension)
                    .await
                    .unwrap_or_else(|e| {
                        Err(genubar_core::MenuError::IoPermission(format!(
                            "Task error: {e:?}"
                        )))
                    });

                btn_c.set_sensitive(true);
                match res {
                    Ok(ver) => {
                        let fresh = UpdateService::inspect_installed_extension();
                        ext_row_in.set_title(&format!(
                            "Extension Version: v{} (Expected: v{EXPECTED_EXTENSION_VERSION})",
                            fresh.version
                        ));
                        ext_row_in.set_subtitle(&fresh.status_summary);
                        ext_row_in.set_tooltip_text(Some(&format!(
                            "Installed v{} • Min Daemon v{} • Tested Daemon v{} • IPC v{}",
                            fresh.version,
                            fresh.min_daemon_version,
                            fresh.tested_daemon_version,
                            fresh.ipc_protocol_version,
                        )));
                        btn_c.set_label("Install / Sync Local");
                        btn_c.remove_css_class("suggested-action");
                        st_in.notify(&format!(
                            "Extension v{ver} installed to ~/.local/share/gnome-shell/extensions/{EXTENSION_UUID}. Log out and back in to reload GNOME Shell."
                        ));
                    }
                    Err(e) => {
                        btn_c.set_label("Install Extension");
                        st_in.notify(&format!("Extension install error: {e}"));
                    }
                }
            });
        });
    }

    {
        let st = state.clone();
        let ext_row_c = ext_row.clone();
        let ext_raw_c = ext_raw_base_state.clone();
        ext_update_btn.connect_clicked(move |btn| {
            let Some(raw_base) = ext_raw_c.borrow().clone() else {
                return;
            };
            btn.set_sensitive(false);
            btn.set_label("Updating...");
            let btn_c = btn.clone();
            let st_in = st.clone();
            let ext_row_in = ext_row_c.clone();

            gtk::glib::spawn_future_local(async move {
                let res = gtk::gio::spawn_blocking(move || {
                    UpdateService::update_extension_files(&raw_base)
                })
                .await
                .unwrap_or_else(|e| {
                    Err(genubar_core::MenuError::IoPermission(format!(
                        "Task error: {e:?}"
                    )))
                });

                match res {
                    Ok(new_ver) => {
                        let fresh = UpdateService::inspect_installed_extension();
                        ext_row_in.set_title(&format!(
                            "Extension Version: v{} (Expected: v{EXPECTED_EXTENSION_VERSION})",
                            fresh.version
                        ));
                        ext_row_in.set_subtitle(&fresh.status_summary);
                        btn_c.set_visible(false);
                        st_in.notify(&format!(
                            "Extension updated to v{new_ver}. Log out and back in to activate."
                        ));
                    }
                    Err(e) => {
                        btn_c.set_sensitive(true);
                        btn_c.set_label("Update Extension");
                        st_in.notify(&format!("Extension update error: {e}"));
                    }
                }
            });
        });
    }

    {
        let ext_rel_url_c = ext_release_url_state.clone();
        ext_release_btn.connect_clicked(move |_| {
            let url = ext_rel_url_c.borrow().clone();
            open_external(&url);
        });
    }

    ext_row.add_suffix(&ext_sync_btn);
    ext_row.add_suffix(&ext_update_btn);
    ext_row.add_suffix(&ext_release_btn);
    ext_group.add(&ext_row);

    let ext_help_row = build_action_row(
        "How Extension Updates Work",
        "Use 'Install Extension' / 'Install / Sync Local' to install local files, run 'genubar-installer' for full setup, or click Check for Updates (↻) above to fetch the latest compatible extension from GitHub. Log out and back in if GNOME Shell already loaded an older version.",
    );
    ext_group.add(&ext_help_row);

    let ext_repo_row = build_action_row(
        "Extension Repository (genubar-gnome-extension)",
        "Standalone GNOME Shell extension repository under github.com/genubar",
    );
    let open_ext_repo_btn = gtk::Button::builder()
        .label("Open GitHub ↗")
        .valign(gtk::Align::Center)
        .build();
    open_ext_repo_btn.connect_clicked(move |_| {
        open_external("https://github.com/genubar/genubar-gnome-extension");
    });
    ext_repo_row.add_suffix(&open_ext_repo_btn);
    ext_group.add(&ext_repo_row);

    ExtensionWidgets {
        group: ext_group,
        ext_row,
        update_btn: ext_update_btn,
        release_btn: ext_release_btn,
        raw_base_state: ext_raw_base_state,
        release_url_state: ext_release_url_state,
    }
}
