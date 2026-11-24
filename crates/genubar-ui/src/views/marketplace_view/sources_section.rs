use crate::components::controls::build_icon_button;
use crate::components::dialogs::show_text_prompt_modal;
use crate::components::rows::{build_action_row, build_two_row_entry};
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use adw::prelude::*;
use genubar_packages::RegistrySourceStatus;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

const LOCAL_EXAMPLE_REGISTRY: &str = "~/.config/genubar/example_registry.json";

pub fn build_registry_sources_group(
    state: &AppState,
    statuses: &[RegistrySourceStatus],
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::PreferencesGroup {
    let reg_group = adw::PreferencesGroup::builder()
        .title("Marketplace Registry Sources (registry.json)")
        .description(
            "Manage community and local JSON package index files (app updates use app.json in settings.toml)",
        )
        .build();

    let urls = state.config.borrow().settings.general.registry_urls.clone();
    let has_local_example = urls.iter().any(|u| u.contains("example_registry.json"));

    for (idx, url) in urls.into_iter().enumerate() {
        let status_note = statuses
            .iter()
            .find(|s| s.source == url)
            .map(|s| match &s.error {
                Some(err) => format!(" • Error: {err}"),
                None => format!(" • Loaded {} package(s)", s.loaded_count),
            })
            .unwrap_or_default();

        let row = build_action_row(
            &format!("Source #{}", idx + 1),
            &format!("{url}{status_note}"),
        );
        let edit_btn = build_icon_button("document-edit-symbolic", "Edit URL", &[]);
        let del_btn =
            build_icon_button("user-trash-symbolic", "Remove URL", &["destructive-action"]);

        {
            let st = state.clone();
            let old_u = url.clone();
            let rh = reload_handle.clone();
            edit_btn.connect_clicked(move |btn| {
                let st_in = st.clone();
                let old_in = old_u.clone();
                let rh_in = rh.clone();
                show_text_prompt_modal(
                    btn,
                    &format!("Edit Registry Source #{}", idx + 1),
                    &old_u,
                    "https://raw.githubusercontent.com/...",
                    "Save URL",
                    move |new_u| match ConfigService::update_registry_url(
                        &mut st_in.config.borrow_mut(),
                        &old_in,
                        &new_u,
                    ) {
                        Ok(true) => {
                            if let Some(ref cb) = *rh_in.borrow() {
                                cb();
                            }
                            true
                        }
                        Ok(false) => false,
                        Err(e) => {
                            st_in.notify(&format!("Failed saving URL: {e}"));
                            false
                        }
                    },
                );
            });
        }
        {
            let st = state.clone();
            let del_u = url;
            let rh = reload_handle.clone();
            del_btn.connect_clicked(move |_| {
                match ConfigService::remove_registry_url(&mut st.config.borrow_mut(), &del_u) {
                    Ok(true) => {
                        if let Some(ref cb) = *rh.borrow() {
                            cb();
                        }
                    }
                    Ok(false) => {}
                    Err(e) => st.notify(&format!("Failed removing URL: {e}")),
                }
            });
        }

        row.add_suffix(&edit_btn);
        row.add_suffix(&del_btn);
        reg_group.add(&row);
    }

    if !has_local_example {
        let sample_row = build_action_row("Local Example Registry", LOCAL_EXAMPLE_REGISTRY);
        let add_sample_btn = gtk::Button::builder()
            .label("+ Add Example Registry")
            .valign(gtk::Align::Center)
            .build();
        {
            let st = state.clone();
            let rh = reload_handle.clone();
            add_sample_btn.connect_clicked(move |_| {
                let mut cfg = st.config.borrow_mut();
                cfg.settings
                    .general
                    .registry_urls
                    .insert(0, String::from(LOCAL_EXAMPLE_REGISTRY));
                let _ = cfg.save_settings();
                drop(cfg);
                if let Some(ref cb) = *rh.borrow() {
                    cb();
                }
            });
        }
        sample_row.add_suffix(&add_sample_btn);
        reg_group.add(&sample_row);
    }

    let (add_url_row, new_url_entry, add_url_btn) = build_two_row_entry(
        "Add Registry Source URL (registry.json)",
        "Paste a raw link or local file:// / ~/.config path to a registry.json file",
        "",
        "https://raw.githubusercontent.com/user/repo/main/registry.json",
        "+ Add Source",
    );
    {
        let st = state.clone();
        let rh = reload_handle.clone();
        add_url_btn.connect_clicked(move |_| {
            let u = new_url_entry.text().trim().to_string();
            if !u.is_empty() {
                let mut cfg = st.config.borrow_mut();
                cfg.settings.general.registry_urls.push(u);
                let _ = cfg.save_settings();
                drop(cfg);
                if let Some(ref cb) = *rh.borrow() {
                    cb();
                }
            }
        });
    }
    reg_group.add(&add_url_row);
    reg_group
}
