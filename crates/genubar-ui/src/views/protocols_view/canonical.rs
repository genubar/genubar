use crate::components::rows::{build_script_selector_row, build_spin_row, build_switch_row};
use crate::state::AppState;
use libadwaita as adw;
use adw::prelude::*;

pub fn build_canonical_group(state: &AppState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Tier 3: Canonical DBusMenu (com.canonical.dbusmenu)")
        .description("Used by VS Code, JetBrains IDEs (RustRover), Qt, and Electron applications")
        .build();

    let cfg = state.config.borrow();
    let can = &cfg.settings.api_tiers.canonical_dbus;
    let enabled = can.enabled;
    let cache_valid = can.cache_last_valid_menu;
    let preload_passes = can.preload_passes as f64;
    let swing_delay = can.swing_open_delay_ms as f64;
    let script = can.python_script.clone();
    drop(cfg);

    group.add(&build_switch_row(
        state,
        "Enable Canonical DBusMenu Tier",
        "Listens for window registrations on com.canonical.AppMenu.Registrar",
        enabled,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .canonical_dbus
                .enabled = active;
        },
    ));

    group.add(&build_switch_row(
        state,
        "Cache Last Valid Menu in RAM & L1 Disk Cache",
        "Prevents Default-Fallback flicker when switching workspaces in Java Swing / JetBrains IDEs",
        cache_valid,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .canonical_dbus
                .cache_last_valid_menu = active;
        },
    ));

    group.add(&build_spin_row(
        state,
        "Submenu Preload Passes",
        "Number of AboutToShow expansion passes performed when a window registers (default: 2)",
        preload_passes,
        0.0,
        5.0,
        1.0,
        |st, val| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .canonical_dbus
                .preload_passes = val as u8;
        },
    ));

    group.add(&build_spin_row(
        state,
        "Java Swing Submenu Open Delay (ms)",
        "Polling interval while waiting for Java Swing EDT to populate a submenu (default: 35ms)",
        swing_delay,
        10.0,
        250.0,
        5.0,
        |st, val| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .canonical_dbus
                .swing_open_delay_ms = val as u64;
        },
    ));

    group.add(&build_script_selector_row(
        state,
        "Canonical DBusMenu Post-Processing Python Script",
        "Optional # ==GenubarScript== executed on Canonical DBusMenu trees",
        script.as_ref(),
        |st, chosen| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .canonical_dbus
                .python_script = chosen;
        },
    ));

    group
}
