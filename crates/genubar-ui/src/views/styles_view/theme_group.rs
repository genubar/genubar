use crate::components::controls::build_compact_dropdown;
use crate::components::dialogs::{show_code_editor_modal, show_text_prompt_modal};
use crate::components::rows::build_action_row;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use adw::prelude::*;
use genubar_config::ThemeMode;
use genubar_platform::expand_tilde;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn build_theme_group(
    state: &AppState,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> adw::PreferencesGroup {
    let theme_group = adw::PreferencesGroup::builder()
        .title("Active Style Preset & Color Scheme")
        .description("Only one style preset is active at a time. Manual edits save to custom.css.")
        .build();

    let styles_list = state.config.borrow().styles.list_styles();
    let style_labels: Vec<String> = styles_list
        .iter()
        .map(|s| format!("{} ({})", s.name, s.file_name))
        .collect();
    let style_refs: Vec<&str> = style_labels.iter().map(String::as_str).collect();

    let cur_active = state.config.borrow().settings.style.active_style.clone();
    let sel_idx = styles_list
        .iter()
        .position(|s| s.file_name == cur_active)
        .unwrap_or(0) as u32;

    let preset_row = build_action_row(
        "Active Style Preset",
        "Select a saved stylesheet from ~/.config/genubar/styles/",
    );
    let preset_drop = build_compact_dropdown(&style_refs, sel_idx);
    {
        let st = state.clone();
        let st_list = styles_list;
        preset_drop.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if let Some(meta) = st_list.get(idx) {
                st.config.borrow_mut().settings.style.active_style = meta.file_name.clone();
                st.save_and_reload();
            }
        });
    }
    preset_row.add_suffix(&preset_drop);
    theme_group.add(&preset_row);

    let actions_row = build_action_row(
        "Stylesheet Actions",
        "Import a new .css preset or edit the active stylesheet code",
    );
    let import_btn = gtk::Button::builder()
        .label("Import .css...")
        .valign(gtk::Align::Center)
        .build();
    let edit_css_btn = gtk::Button::builder()
        .label("Edit Active CSS...")
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();

    {
        let st = state.clone();
        let rh = reload_handle.clone();
        import_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            let rh_in = rh.clone();
            show_text_prompt_modal(
                btn,
                "Import Style Preset (.css)",
                "",
                "~/Downloads/my_theme.css",
                "Import into ~/.config/genubar/styles/",
                move |raw_path| {
                    let p = expand_tilde(&raw_path);
                    let res = st_in.config.borrow().styles.import_style_file(&p);
                    match res {
                        Ok(fname) => {
                            st_in.config.borrow_mut().settings.style.active_style = fname;
                            st_in.save_and_reload();
                            if let Some(ref cb) = *rh_in.borrow() {
                                cb();
                            }
                            true
                        }
                        Err(e) => {
                            st_in.notify(&format!("Import failed: {e}"));
                            false
                        }
                    }
                },
            );
        });
    }

    {
        let st = state.clone();
        let rh = reload_handle.clone();
        edit_css_btn.connect_clicked(move |btn| {
            let raw_css = match ConfigService::read_active_style_css(&st.config.borrow()) {
                Ok(text) => text,
                Err(e) => {
                    st.notify(&format!("Cannot read active CSS: {e}"));
                    return;
                }
            };

            let st_save = st.clone();
            let rh_save = rh.clone();
            let st_rest = st.clone();
            let rh_rest = rh.clone();

            show_code_editor_modal(
                btn,
                "Edit Stylesheet (Saves to styles/custom.css)",
                &raw_css,
                "Save as 'custom.css' & Live-Reload",
                Some("Switch Back to 'default.css'"),
                move |new_text| {
                    let res = st_save
                        .config
                        .borrow()
                        .styles
                        .save_custom_modified_css(&new_text);
                    if let Ok(new_active) = res {
                        st_save.config.borrow_mut().settings.style.active_style = new_active;
                        st_save.save_and_reload();
                        if let Some(ref cb) = *rh_save.borrow() {
                            cb();
                        }
                        true
                    } else {
                        false
                    }
                },
                Some(Box::new(move || {
                    st_rest.config.borrow_mut().settings.style.active_style =
                        String::from("default.css");
                    st_rest.save_and_reload();
                    if let Some(ref cb) = *rh_rest.borrow() {
                        cb();
                    }
                })),
            );
        });
    }

    actions_row.add_suffix(&import_btn);
    actions_row.add_suffix(&edit_css_btn);
    theme_group.add(&actions_row);

    let mode_row = build_action_row(
        "Color Scheme",
        "Auto follows GNOME system theme; Light and Dark force menu colors",
    );
    let mode_drop = build_compact_dropdown(
        &["Auto", "Dark", "Light"],
        match state.config.borrow().settings.style.theme_mode {
            ThemeMode::Auto => 0,
            ThemeMode::Dark => 1,
            ThemeMode::Light => 2,
        },
    );
    {
        let st = state.clone();
        mode_drop.connect_selected_notify(move |dd| {
            st.config.borrow_mut().settings.style.theme_mode = match dd.selected() {
                1 => ThemeMode::Dark,
                2 => ThemeMode::Light,
                _ => ThemeMode::Auto,
            };
            st.save_and_reload();
        });
    }
    mode_row.add_suffix(&mode_drop);
    theme_group.add(&mode_row);

    let panel_row = build_action_row(
        "Recolor Entire GNOME Top Panel in Light Mode",
        "Applies a light background and dark text to #panel when Light Mode is active",
    );
    let panel_sw = gtk::Switch::builder()
        .active(state.config.borrow().settings.style.recolor_gnome_panel)
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        panel_sw.connect_state_set(move |_, active| {
            st.config.borrow_mut().settings.style.recolor_gnome_panel = active;
            st.save_and_reload();
            gtk::glib::Propagation::Proceed
        });
    }
    panel_row.add_suffix(&panel_sw);
    theme_group.add(&panel_row);

    theme_group
}
