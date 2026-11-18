use crate::components::controls::build_compact_dropdown;
use crate::components::rows::{
    build_action_row, build_script_selector_row, build_switch_row, build_two_row_entry,
};
use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use genubar_config::Gtk4LayoutAlgorithm;
use std::path::PathBuf;

pub fn build_gtk4_hook_group(state: &AppState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Tier 2: GTK4 Hook (libgenubar_gtk4_hook.so)")
        .description("Extracts popover menus, Zoom controls, and Adwaita themes from GTK4 apps")
        .build();

    let cfg = state.config.borrow();
    let hook = &cfg.settings.api_tiers.gtk4_hook;
    let enabled = hook.enabled;
    let hide_buttons = hook.hide_original_buttons;
    let expose_unknown = hook.expose_unknown_custom_widgets;
    let cur_alg = hook.layout_algorithm;
    let cur_script = hook.python_script.clone();
    let cur_so = hook
        .custom_hook_so_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    drop(cfg);

    group.add(&build_switch_row(
        state,
        "Enable GTK4 Hook Tier",
        "Accepts menu registrations from GTK4 applications launched with active_gtk4_hook.so",
        enabled,
        |st, active| {
            st.config.borrow_mut().settings.api_tiers.gtk4_hook.enabled = active;
        },
    ));

    group.add(&build_switch_row(
        state,
        "Hide Original Hamburger Buttons Inside GTK4 Windows",
        "Updates GENUBAR_GTK4_HIDE_BUTTONS in session RAM immediately",
        hide_buttons,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .hide_original_buttons = active;
        },
    ));

    group.add(&build_switch_row(
        state,
        "Expose Unrecognized Custom Popover Widgets",
        "Places custom GTK4 widget slots that cannot be expanded into View",
        expose_unknown,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .expose_unknown_custom_widgets = active;
        },
    ));

    let alg_row = build_action_row(
        "GTK4 Menu Organization",
        "Standard splits flat hamburger menus into File | Edit | View | Help",
    );
    let alg_drop = build_compact_dropdown(
        &["Standard", "Original", "Python Script"],
        match cur_alg {
            Gtk4LayoutAlgorithm::Standard => 0,
            Gtk4LayoutAlgorithm::Original => 1,
            Gtk4LayoutAlgorithm::PythonScript => 2,
        },
    );
    alg_row.add_suffix(&alg_drop);
    group.add(&alg_row);

    let g4_script_row = build_script_selector_row(
        state,
        "GTK4 Layout Python Script",
        "Select a # ==GenubarScript== file to organize GTK4 popover items",
        cur_script.as_ref(),
        |st, chosen| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .python_script = chosen;
        },
    );
    g4_script_row.set_visible(cur_alg == Gtk4LayoutAlgorithm::PythonScript);

    {
        let st = state.clone();
        let g4_sr = g4_script_row.clone();
        alg_drop.connect_selected_notify(move |dd| {
            let alg = match dd.selected() {
                1 => Gtk4LayoutAlgorithm::Original,
                2 => Gtk4LayoutAlgorithm::PythonScript,
                _ => Gtk4LayoutAlgorithm::Standard,
            };
            g4_sr.set_visible(alg == Gtk4LayoutAlgorithm::PythonScript);
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .layout_algorithm = alg;
            st.save_and_reload();
        });
    }
    group.add(&g4_script_row);

    let (so_row, so_entry, so_btn) = build_two_row_entry(
        "Custom GTK4 Hook Library Path (.so)",
        "Updates ~/.local/lib/genubar/active_gtk4_hook.so symlink. Use 'Reset to Default .so' to restore built-in hook.",
        &cur_so,
        "Default (built-in libgenubar_gtk4_hook.so)",
        "Apply .so Path",
    );
    let reset_so_btn = gtk::Button::builder()
        .label("Reset to Default .so")
        .valign(gtk::Align::Center)
        .build();
    if let Some(outer_box) = so_row.child().and_then(|w| w.downcast::<gtk::Box>().ok())
        && let Some(input_box) = outer_box
            .last_child()
            .and_then(|w| w.downcast::<gtk::Box>().ok())
        {
            input_box.append(&reset_so_btn);
        }

    {
        let st = state.clone();
        let se = so_entry.clone();
        so_btn.connect_clicked(move |_| {
            let txt = se.text().trim().to_string();
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .custom_hook_so_path = if txt.is_empty() {
                None
            } else {
                Some(PathBuf::from(txt))
            };
            st.save_and_reload();
        });
    }
    {
        let st = state.clone();
        let se = so_entry;
        reset_so_btn.connect_clicked(move |_| {
            se.set_text("");
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk4_hook
                .custom_hook_so_path = None;
            st.save_and_reload();
            st.notify("Restored built-in libgenubar_gtk4_hook.so");
        });
    }
    group.add(&so_row);

    group
}
