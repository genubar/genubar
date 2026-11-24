use crate::components::rows::{
    build_script_selector_row, build_spin_row, build_switch_row, build_two_row_entry,
};
use crate::state::AppState;
use crate::utils::text::{format_csv_list, parse_csv_list};
use adw::prelude::*;
use libadwaita as adw;

pub fn build_atspi_group(state: &AppState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Tier 4: AT-SPI2 Accessibility Bus")
        .description("Extracts and caches menus from Firefox and Chromium-based browsers")
        .build();

    let cfg = state.config.borrow();
    let atspi = &cfg.settings.api_tiers.atspi;
    let enabled = atspi.enabled;
    let disk_cache = atspi.enable_disk_cache;
    let max_depth = atspi.search.max_depth as f64;
    let max_budget = atspi.search.max_nodes_budget as f64;
    let volatiles_csv = format_csv_list(&atspi.default_volatile_submenus);
    let script = atspi.python_script.clone();
    drop(cfg);

    group.add(&build_switch_row(
        state,
        "Enable AT-SPI2 Tier",
        "Queries org.a11y.atspi.Accessible hierarchy when no D-Bus menu is exported",
        enabled,
        |st, active| {
            st.config.borrow_mut().settings.api_tiers.atspi.enabled = active;
        },
    ));

    group.add(&build_switch_row(
        state,
        "Enable Persistent L2 Disk Cache",
        "Caches expanded submenus in ~/.cache/genubar/atspi_menus.json for instant opening",
        disk_cache,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .atspi
                .enable_disk_cache = active;
        },
    ));

    group.add(&build_spin_row(
        state,
        "Maximum Tree Search Depth",
        "Maximum BFS recursion depth when searching for a MenuBar role (default: 6)",
        max_depth,
        3.0,
        25.0,
        1.0,
        |st, val| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .atspi
                .search
                .max_depth = val as usize;
        },
    ));

    group.add(&build_spin_row(
        state,
        "Maximum Nodes Budget",
        "Upper limit on inspected accessibility nodes per window focus (default: 85)",
        max_budget,
        30.0,
        2000.0,
        25.0,
        |st, val| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .atspi
                .search
                .max_nodes_budget = val as usize;
        },
    ));

    let (vol_row, vol_entry, vol_btn) = build_two_row_entry(
        "Default Volatile Submenus",
        "Comma-separated submenu names refreshed on focus instead of frozen in L2 cache",
        &volatiles_csv,
        "History, Bookmarks",
        "Apply Volatiles",
    );
    {
        let st = state.clone();
        vol_btn.connect_clicked(move |_| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .atspi
                .default_volatile_submenus = parse_csv_list(&vol_entry.text());
            st.save_and_reload();
        });
    }
    group.add(&vol_row);

    group.add(&build_script_selector_row(
        state,
        "AT-SPI2 Post-Processing Python Script",
        "Optional # ==GenubarScript== executed on AT-SPI2 trees",
        script.as_ref(),
        |st, chosen| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .atspi
                .python_script = chosen;
        },
    ));

    group
}
