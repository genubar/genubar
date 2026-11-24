use crate::components::controls::{
    build_caption_label, build_compat_warning_label, build_icon_button, build_popover_menu_button,
};
use crate::components::dialogs::show_info_modal;
use crate::components::rows::build_action_row;
use crate::models::info_item::InfoField;
use crate::models::package_ui::{package_kind_badge, package_kind_tab_id, package_kind_tab_title};
use crate::state::AppState;
use adw::prelude::*;
use genubar_packages::{
    CatalogSearchItem, RegistryPackageInstaller, RemotePackageEntry, UpdateService,
};
use gtk4 as gtk;
use libadwaita as adw;

fn build_package_info_button(pkg: &RemotePackageEntry) -> gtk::Button {
    let info_btn = build_icon_button("dialog-information-symbolic", "Package Details", &[]);
    let pkg_c = pkg.clone();
    info_btn.connect_clicked(move |btn| {
        let mut fields = vec![
            InfoField::new("Package ID", pkg_c.id.clone()),
            InfoField::new("Type", package_kind_badge(pkg_c.kind).to_string()),
            InfoField::new("Name", pkg_c.name.clone()),
            InfoField::new("Version", pkg_c.version.clone()),
            InfoField::new("Author", pkg_c.author.clone()),
            InfoField::new(
                "Min App Version",
                pkg_c
                    .min_app_version
                    .clone()
                    .unwrap_or_else(|| String::from("Any")),
            ),
            InfoField::new("Tags", pkg_c.tags.join(", ")),
            InfoField::new("Description", pkg_c.description.clone()),
            InfoField::new("Source", pkg_c.source_url.clone()),
        ];
        if let Some(reason) = pkg_c.incompatibility_badge() {
            fields.push(InfoField::new("Compatibility Notice", reason));
        }
        show_info_modal(btn, &format!("Package: {}", pkg_c.name), &fields);
    });
    info_btn
}

fn spawn_remote_install(
    btn: &gtk::Button,
    state: &AppState,
    pkg: &RemotePackageEntry,
    allow_incompatible: bool,
    original_label: &'static str,
) {
    btn.set_sensitive(false);
    btn.set_label(if allow_incompatible {
        "Saving..."
    } else {
        "Installing..."
    });
    let btn_c = btn.clone();
    let st_in = state.clone();
    let pkg_in = pkg.clone();

    gtk::glib::spawn_future_local(async move {
        let pkg_for_fetch = pkg_in.clone();
        let fetch_res = gtk::gio::spawn_blocking(move || {
            UpdateService::fetch_package_payload_ext(&pkg_for_fetch, allow_incompatible)
        })
        .await
        .unwrap_or_else(|e| {
            Err(genubar_core::MenuError::IoPermission(format!(
                "Task error: {e:?}"
            )))
        });

        match fetch_res {
            Ok(payload) => {
                let res = {
                    let mut cfg = st_in.config.borrow_mut();
                    let mut patcher = st_in.patcher.borrow_mut();
                    RegistryPackageInstaller::install_fetched_package_ext(
                        &mut cfg,
                        &mut patcher,
                        &pkg_in,
                        payload,
                        allow_incompatible,
                    )
                };
                match res {
                    Ok(msg) => {
                        st_in.notify(&msg);
                        st_in.save_and_reload();
                        st_in.rebuild_all_views();
                        return;
                    }
                    Err(e) => st_in.notify(&format!("Install error: {e}")),
                }
            }
            Err(e) => st_in.notify(&format!("Download error: {e}")),
        }

        btn_c.set_sensitive(true);
        btn_c.set_label(original_label);
    });
}

pub fn build_search_result_row(state: &AppState, item: &CatalogSearchItem) -> adw::ActionRow {
    let r = build_action_row(
        &item.title,
        &format!("{} • {}", package_kind_badge(item.kind), item.subtitle),
    );
    r.set_title_lines(1);
    r.set_subtitle_lines(2);

    if !item.is_compatible {
        let req_text = item
            .incompatibility_badge
            .as_deref()
            .unwrap_or("Update Required");
        r.add_suffix(&build_caption_label(req_text, Some("dim-label")));
    } else if item.is_local {
        let tab = package_kind_tab_id(item.kind);
        let open_btn = gtk::Button::builder()
            .label("Open Tab ↗")
            .valign(gtk::Align::Center)
            .build();
        let st = state.clone();
        open_btn.connect_clicked(move |_| st.navigate_to(tab));
        r.add_suffix(&open_btn);
    }
    r
}

pub fn build_remote_package_row(state: &AppState, pkg: &RemotePackageEntry) -> adw::ActionRow {
    let compat = pkg.is_compatible();
    let badge_opt = pkg.incompatibility_badge();
    let is_installed = {
        let cfg = state.config.borrow();
        let patcher = state.patcher.borrow();
        RegistryPackageInstaller::is_package_installed(&cfg, &patcher, pkg)
    };
    let tab_id = package_kind_tab_id(pkg.kind);
    let tab_title = package_kind_tab_title(pkg.kind);

    let r = build_action_row(
        &pkg.name,
        &format!(
            "{} • v{} • by {} — {}",
            package_kind_badge(pkg.kind),
            pkg.version,
            pkg.author,
            pkg.description
        ),
    );
    r.set_title_lines(1);
    r.set_subtitle_lines(2);

    if is_installed {
        r.add_suffix(&build_caption_label(
            &format!("Saved in {tab_title}"),
            Some("accent"),
        ));
    } else if !compat {
        let req_text = badge_opt.as_deref().unwrap_or("Update Required");
        r.add_suffix(&build_caption_label(req_text, Some("dim-label")));
    }

    r.add_suffix(&build_package_info_button(pkg));

    if is_installed {
        let open_btn = gtk::Button::builder()
            .label(format!("Open in {tab_title} ↗"))
            .valign(gtk::Align::Center)
            .build();
        {
            let st = state.clone();
            open_btn.connect_clicked(move |_| st.navigate_to(tab_id));
        }
        r.add_suffix(&open_btn);

        let (more_btn, pop, pop_box) = build_popover_menu_button("More Options", 6);
        let reinstall_btn = gtk::Button::builder()
            .label("Reinstall / Overwrite")
            .css_classes(["flat"])
            .build();
        {
            let st = state.clone();
            let pkg_c = pkg.clone();
            let pop_c = pop.clone();
            reinstall_btn.connect_clicked(move |btn| {
                pop_c.popdown();
                spawn_remote_install(btn, &st, &pkg_c, !compat, "Reinstall / Overwrite");
            });
        }
        pop_box.append(&reinstall_btn);
        r.add_suffix(&more_btn);
    } else if compat {
        let install_btn = gtk::Button::builder()
            .label("Install")
            .valign(gtk::Align::Center)
            .css_classes(["suggested-action"])
            .build();
        let st = state.clone();
        let pkg_c = pkg.clone();
        install_btn.connect_clicked(move |btn| {
            spawn_remote_install(btn, &st, &pkg_c, false, "Install");
        });
        r.add_suffix(&install_btn);
    } else {
        let (more_btn, pop, pop_box) = build_popover_menu_button("More Options", 8);
        let warn_lbl = build_compat_warning_label(
            badge_opt
                .as_deref()
                .unwrap_or("Requires a newer Genubar release"),
        );
        let force_dl_btn = gtk::Button::builder()
            .label("Download & Save Anyway")
            .css_classes(["flat"])
            .tooltip_text("Saves the raw file/manifest locally without enabling incompatible items")
            .build();

        pop_box.append(&warn_lbl);
        pop_box.append(&force_dl_btn);

        {
            let st = state.clone();
            let pkg_c = pkg.clone();
            let pop_c = pop.clone();
            force_dl_btn.connect_clicked(move |btn| {
                pop_c.popdown();
                spawn_remote_install(btn, &st, &pkg_c, true, "Download & Save Anyway");
            });
        }

        r.add_suffix(&more_btn);
    }

    r
}
