use crate::components::controls::build_compact_dropdown;
use crate::components::rows::build_action_row;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use crate::utils::system::open_directory;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::ActionRowExt;
use std::path::PathBuf;

pub fn build_script_selector_row(
    state: &AppState,
    title: &str,
    subtitle: &str,
    current_script: Option<&PathBuf>,
    on_select: impl Fn(&AppState, Option<PathBuf>) + 'static,
) -> adw::ActionRow {
    let row = build_action_row(title, subtitle);

    let mut options = vec![String::from("None (Disabled)")];
    let discovered = ConfigService::list_valid_scripts(&state.config.borrow());
    options.extend(discovered);

    let current_str = current_script
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let selected_idx = if current_str.is_empty() {
        0
    } else {
        options
            .iter()
            .position(|s| *s == current_str)
            .unwrap_or_else(|| {
                options.push(current_str.clone());
                options.len() - 1
            })
    } as u32;

    let opt_refs: Vec<&str> = options.iter().map(String::as_str).collect();
    let drop = build_compact_dropdown(&opt_refs, selected_idx);

    let open_folder_btn = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Open ~/.config/genubar/scripts/ folder")
        .valign(gtk::Align::Center)
        .build();

    {
        let st = state.clone();
        open_folder_btn.connect_clicked(move |_| {
            let dir = st.config.borrow().base_dir().join("scripts");
            open_directory(&dir);
        });
    }

    {
        let st = state.clone();
        let opts_clone = options.clone();
        drop.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            let chosen = if idx == 0 {
                None
            } else {
                opts_clone.get(idx).map(PathBuf::from)
            };
            on_select(&st, chosen);
            st.save_and_reload();
        });
    }

    row.add_suffix(&drop);
    row.add_suffix(&open_folder_btn);
    row
}
