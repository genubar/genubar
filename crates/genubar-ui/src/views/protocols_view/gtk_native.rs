use crate::components::rows::{build_script_selector_row, build_switch_row};
use crate::state::AppState;
use libadwaita as adw;
use adw::prelude::*;

pub fn build_gtk_native_group(state: &AppState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Tier 1: GTK Native D-Bus Menus")
        .description("Used by native GTK3 applications that export GMenuModel directly to the session bus")
        .build();

    let cfg = state.config.borrow();
    let native = &cfg.settings.api_tiers.gtk_native;
    let enabled = native.enabled;
    let include_app = native.include_app_menu_column;
    let script = native.python_script.clone();
    drop(cfg);

    group.add(&build_switch_row(
        state,
        "Enable GTK Native Tier",
        "Reads org.gtk.Menus and org.gtk.Actions paths advertised by the window",
        enabled,
        |st, active| {
            st.config.borrow_mut().settings.api_tiers.gtk_native.enabled = active;
        },
    ));

    group.add(&build_switch_row(
        state,
        "Include Native GTK AppMenu Column",
        "Merges the application-wide menu object path alongside the window menubar",
        include_app,
        |st, active| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk_native
                .include_app_menu_column = active;
        },
    ));

    group.add(&build_script_selector_row(
        state,
        "GTK Native Post-Processing Python Script",
        "Optional # ==GenubarScript== executed on GTK Native menus",
        script.as_ref(),
        |st, chosen| {
            st.config
                .borrow_mut()
                .settings
                .api_tiers
                .gtk_native
                .python_script = chosen;
        },
    ));

    group
}
