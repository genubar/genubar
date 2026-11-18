mod extension_group;

use crate::components::controls::build_icon_button;
use crate::components::rows::build_action_row;
use genubar_packages::{ExtensionReleaseManifest, UpdateCheckStatus, UpdateService};
use crate::state::AppState;
use crate::utils::system::open_external;
use extension_group::build_extension_group;
use genubar_core::{CURRENT_APP_VERSION, is_version_compatible};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_about_view(state: &AppState) -> gtk::Widget {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();

    let info_group = adw::PreferencesGroup::builder()
        .title("Genubar — Universal Global Menubar for GNOME")
        .description(
            "Unifies GTK Native, GTK4 LD_PRELOAD Hook, Canonical DBusMenu, and AT-SPI2 into a customizable top bar.",
        )
        .build();

    let ver_row = build_action_row(
        &format!("Checking Daemon Status... (Settings UI: v{CURRENT_APP_VERSION})"),
        "Querying installed binary and systemd service",
    );
    let brand_icon = if let Ok(pb) = gtk::gdk::gdk_pixbuf::Pixbuf::from_read(
        std::io::Cursor::new(genubar_platform::EMBEDDED_APP_ICON_SVG.as_bytes()),
    ) {
        gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pb)))
    } else {
        gtk::Image::from_icon_name(genubar_platform::APP_ICON_NAME)
    };
    brand_icon.set_pixel_size(36);
    ver_row.add_prefix(&brand_icon);
    {
        let ver_row_init = ver_row.clone();
        gtk::glib::spawn_future_local(async move {
            if let Ok(daemon_info) =
                gtk::gio::spawn_blocking(UpdateService::inspect_installed_daemon).await
            {
                let daemon_title = if daemon_info.installed {
                    format!(
                        "Daemon Version: v{} (Settings UI: v{CURRENT_APP_VERSION})",
                        daemon_info.version
                    )
                } else {
                    format!("Daemon Not Installed (Settings UI: v{CURRENT_APP_VERSION})")
                };
                ver_row_init.set_title(&daemon_title);
                ver_row_init.set_subtitle(&daemon_info.status_summary);
                if daemon_info.installed {
                    ver_row_init.set_tooltip_text(Some(if daemon_info.service_active {
                        "genubar.service is currently active"
                    } else {
                        "genubar.service is stopped"
                    }));
                }
            }
        });
    }

    let check_btn = build_icon_button(
        "view-refresh-symbolic",
        "Check app.json for Genubar & GNOME Extension updates",
        &[],
    );

    let release_btn = gtk::Button::builder()
        .label("Update Available — Open Releases ↗")
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .visible(false)
        .build();

    let release_url_state = Rc::new(RefCell::new(String::from(
        "https://github.com/genubar/genubar/releases",
    )));

    let ext = build_extension_group(state);

    {
        let st = state.clone();
        let ver_row_c = ver_row.clone();
        let rel_btn_c = release_btn.clone();
        let rel_url_c = release_url_state.clone();
        let ext_row_c = ext.ext_row.clone();
        let ext_upd_btn_c = ext.update_btn.clone();
        let ext_rel_btn_c = ext.release_btn.clone();
        let ext_raw_c = ext.raw_base_state.clone();
        let ext_rel_url_c = ext.release_url_state.clone();

        check_btn.connect_clicked(move |btn| {
            let app_url = st.config.borrow().settings.general.app_update_url.clone();
            let reg_urls = st.config.borrow().settings.general.registry_urls.clone();
            btn.set_sensitive(false);
            let btn_c = btn.clone();
            let st_in = st.clone();
            let ver_row_in = ver_row_c.clone();
            let rel_btn_in = rel_btn_c.clone();
            let rel_url_in = rel_url_c.clone();
            let ext_row_in = ext_row_c.clone();
            let ext_upd_in = ext_upd_btn_c.clone();
            let ext_rel_in = ext_rel_btn_c.clone();
            let ext_raw_in = ext_raw_c.clone();
            let ext_rel_url_in = ext_rel_url_c.clone();

            gtk::glib::spawn_future_local(async move {
                let status = gtk::gio::spawn_blocking(move || {
                    UpdateService::check_for_app_updates(&app_url, &reg_urls)
                })
                .await
                .unwrap_or(UpdateCheckStatus::OfflineOrUnreachable);

                btn_c.set_sensitive(true);
                let mut ext_manifest_opt: Option<ExtensionReleaseManifest> = None;

                match status {
                    UpdateCheckStatus::UpdateAvailable(info) => {
                        ext_manifest_opt = info.extension;
                        if let Some(url) = info.download_url {
                            *rel_url_in.borrow_mut() = url;
                        }
                        let notes = if info.changelog.is_empty() {
                            String::from("Download genubar-installer from Releases.")
                        } else {
                            info.changelog.join(" • ")
                        };
                        ver_row_in.set_subtitle(&format!(
                            "New Daemon v{} is available! {notes}",
                            info.version
                        ));
                        rel_btn_in.set_visible(true);
                        st_in.notify(&format!("Update available: v{}", info.version));
                    }
                    UpdateCheckStatus::UpToDate { extension } => {
                        ext_manifest_opt = extension;
                        ver_row_in.set_subtitle(&format!(
                            "Genubar v{CURRENT_APP_VERSION} is up to date"
                        ));
                        rel_btn_in.set_visible(false);
                        st_in.notify(&format!("Genubar v{CURRENT_APP_VERSION} is up to date"));
                    }
                    UpdateCheckStatus::OfflineOrUnreachable => {
                        ver_row_in.set_subtitle(&format!(
                            "Genubar v{CURRENT_APP_VERSION} (offline or app.json unreachable)"
                        ));
                        st_in.notify(&format!(
                            "Genubar v{CURRENT_APP_VERSION} (offline or app.json unreachable)"
                        ));
                    }
                }

                if let Some(ext_man) = ext_manifest_opt {
                    let cur_ext = UpdateService::inspect_installed_extension();
                    if let Some(ref dl) = ext_man.download_url {
                        *ext_rel_url_in.borrow_mut() = dl.clone();
                    }
                    let newer_ext_available = !cur_ext.installed
                        || (!ext_man.latest_version.trim().is_empty()
                            && !is_version_compatible(&ext_man.latest_version, &cur_ext.version));

                    if newer_ext_available {
                        if ext_man.is_compatible_with_current_daemon() {
                            ext_row_in.set_subtitle(&format!(
                                "Extension update v{} available (compatible with Daemon v{CURRENT_APP_VERSION})",
                                ext_man.latest_version
                            ));
                            if let Some(raw_base) = ext_man.raw_base_url {
                                *ext_raw_in.borrow_mut() = Some(raw_base);
                                ext_upd_in.set_label(&format!("Install v{}", ext_man.latest_version));
                                ext_upd_in.set_visible(true);
                                ext_rel_in.set_visible(false);
                            } else {
                                ext_rel_in.set_visible(true);
                            }
                        } else {
                            ext_row_in.set_subtitle(&format!(
                                "Extension v{} released, but requires Daemon v{}+ (update Genubar first)",
                                ext_man.latest_version,
                                ext_man.min_daemon_version.as_deref().unwrap_or("newer")
                            ));
                            ext_upd_in.set_visible(false);
                            ext_rel_in.set_visible(true);
                        }
                    } else if cur_ext.is_compatible {
                        ext_upd_in.set_visible(false);
                        ext_rel_in.set_visible(false);
                    }
                }
            });
        });
    }

    {
        let rel_url_c = release_url_state;
        release_btn.connect_clicked(move |_| {
            let url = rel_url_c.borrow().clone();
            open_external(&url);
        });
    }

    ver_row.add_suffix(&check_btn);
    ver_row.add_suffix(&release_btn);
    info_group.add(&ver_row);

    let repo_row = build_action_row(
        "Core Repository (genubar)",
        "Daemon, GTK4 Hook, Settings UI, and Patch Engine on GitHub",
    );
    let open_repo_btn = gtk::Button::builder()
        .label("Open GitHub ↗")
        .valign(gtk::Align::Center)
        .build();
    open_repo_btn.connect_clicked(move |_| {
        open_external("https://github.com/genubar/genubar");
    });
    repo_row.add_suffix(&open_repo_btn);
    info_group.add(&repo_row);

    page.add(&info_group);
    page.add(&ext.group);
    scrolled.set_child(Some(&page));
    scrolled.upcast()
}
