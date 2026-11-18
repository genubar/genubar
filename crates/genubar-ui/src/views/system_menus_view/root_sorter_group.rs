use crate::components::controls::build_compact_dropdown;
use crate::components::dialogs::show_exclusions_manager_modal;
use crate::components::rows::{build_action_row, build_script_selector_row, build_two_row_entry};
use crate::state::AppState;
use crate::utils::text::{format_csv_list, parse_csv_list};
use genubar_config::RootSortMode;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub(super) fn build_root_sorter_group(state: &AppState) -> adw::PreferencesGroup {
    let sort_group = adw::PreferencesGroup::builder()
        .title("Root Menu Column Sorter")
        .description("Automatically orders top-level tabs across applications")
        .build();

    let cur_mode = state.config.borrow().settings.root_sorting.mode;

    let mode_row = build_action_row(
        "Root Sorting Mode",
        "Standard enforces File > Edit > View > * > Tools > Window > Help",
    );
    let mode_drop = build_compact_dropdown(
        &["Disabled", "Standard", "Custom", "Python Script"],
        match cur_mode {
            RootSortMode::Disabled => 0,
            RootSortMode::Standard => 1,
            RootSortMode::Custom => 2,
            RootSortMode::PythonScript => 3,
        },
    );
    mode_row.add_suffix(&mode_drop);
    sort_group.add(&mode_row);

    let (custom_order_row, order_entry, order_save_btn) = build_two_row_entry(
        "Custom Column Order",
        "Comma-separated list of column titles. Use '*' to place all remaining application tabs.",
        &format_csv_list(&state.config.borrow().settings.root_sorting.custom_order),
        "{app_name}, File, Edit, View, *, Window, Help",
        "Apply Order",
    );
    custom_order_row.set_visible(cur_mode == RootSortMode::Custom);

    let cur_sort_script = state
        .config
        .borrow()
        .settings
        .root_sorting
        .python_script
        .clone();
    let sort_script_row = build_script_selector_row(
        state,
        "Root Sorting Python Script",
        "Select a # ==GenubarScript== file from ~/.config/genubar/scripts/",
        cur_sort_script.as_ref(),
        |st, chosen| {
            st.config.borrow_mut().settings.root_sorting.python_script = chosen;
        },
    );
    sort_script_row.set_visible(cur_mode == RootSortMode::PythonScript);

    {
        let st = state.clone();
        let custom_row_c = custom_order_row.clone();
        let script_row_c = sort_script_row.clone();
        mode_drop.connect_selected_notify(move |dd| {
            let m = match dd.selected() {
                0 => RootSortMode::Disabled,
                2 => RootSortMode::Custom,
                3 => RootSortMode::PythonScript,
                _ => RootSortMode::Standard,
            };
            custom_row_c.set_visible(m == RootSortMode::Custom);
            script_row_c.set_visible(m == RootSortMode::PythonScript);
            st.config.borrow_mut().settings.root_sorting.mode = m;
            st.save_and_reload();
        });
    }
    {
        let st = state.clone();
        let oe = order_entry;
        order_save_btn.connect_clicked(move |_| {
            st.config.borrow_mut().settings.root_sorting.custom_order =
                parse_csv_list(&oe.text());
            st.save_and_reload();
        });
    }
    sort_group.add(&custom_order_row);
    sort_group.add(&sort_script_row);

    let excl_count = state
        .config
        .borrow()
        .settings
        .root_sorting
        .exclude_apps
        .len();
    let excl_row = build_action_row(
        "Excluded Applications",
        &format!("{excl_count} application pattern(s) excluded from root tab sorting"),
    );
    let open_excl_btn = gtk::Button::builder()
        .label("Open Exclusions...")
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        let excl_row_c = excl_row.clone();
        open_excl_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            let row_in = excl_row_c.clone();
            show_exclusions_manager_modal(btn, &st, move || {
                let c = st_in
                    .config
                    .borrow()
                    .settings
                    .root_sorting
                    .exclude_apps
                    .len();
                row_in.set_subtitle(&format!(
                    "{c} application pattern(s) excluded from root tab sorting"
                ));
            });
        });
    }
    excl_row.add_suffix(&open_excl_btn);
    sort_group.add(&excl_row);
    sort_group
}
