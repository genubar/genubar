use crate::components::controls::build_spin_button;
use crate::components::rows::build_action_row;
use crate::state::AppState;
use adw::prelude::*;
use genubar_config::StyleFileConfig;
use libadwaita as adw;

type MetricUpdater = fn(&mut StyleFileConfig, u32);

struct MetricRowSpec {
    title: &'static str,
    subtitle: &'static str,
    value: u32,
    min: f64,
    max: f64,
    updater: MetricUpdater,
}

pub(super) fn build_metrics_group(state: &AppState) -> adw::PreferencesGroup {
    let metrics_group = adw::PreferencesGroup::builder()
        .title("Dimensions, Spacing & Corner Radius Metrics")
        .description("Changes are debounced and hot-reloaded into GNOME Shell automatically")
        .build();

    let s = state.config.borrow().settings.style.clone();
    let specs = [
        MetricRowSpec {
            title: "Popup Menu Border Radius (px)",
            subtitle: "Corner roundness of the outer dropdown menu card (default: 14px)",
            value: s.popup_border_radius_px,
            min: 0.0,
            max: 32.0,
            updater: |st, v| st.popup_border_radius_px = v,
        },
        MetricRowSpec {
            title: "Menu Item & Search Border Radius (px)",
            subtitle: "Concentric corner roundness of hovered buttons and search bar (default: 9px)",
            value: s.item_border_radius_px,
            min: 0.0,
            max: 24.0,
            updater: |st, v| st.item_border_radius_px = v,
        },
        MetricRowSpec {
            title: "Font Size (px)",
            subtitle: "Text size for menu item labels (default: 13px)",
            value: s.font_size_px,
            min: 10.0,
            max: 24.0,
            updater: |st, v| st.font_size_px = v,
        },
        MetricRowSpec {
            title: "Top Panel Button Horizontal Padding (px)",
            subtitle: "Spacing between top-level application tabs on the GNOME panel (default: 6px)",
            value: s.panel_button_hpadding_px,
            min: 2.0,
            max: 24.0,
            updater: |st, v| st.panel_button_hpadding_px = v,
        },
        MetricRowSpec {
            title: "Menu Item Vertical Padding (px)",
            subtitle: "Top and bottom padding inside each menu row (default: 5px)",
            value: s.item_vpadding_px,
            min: 1.0,
            max: 18.0,
            updater: |st, v| st.item_vpadding_px = v,
        },
        MetricRowSpec {
            title: "Menu Item Horizontal Padding (px)",
            subtitle: "Left and right padding inside each menu row (default: 10px)",
            value: s.item_hpadding_px,
            min: 4.0,
            max: 28.0,
            updater: |st, v| st.item_hpadding_px = v,
        },
        MetricRowSpec {
            title: "Minimum Submenu Column Width (px)",
            subtitle: "Minimum horizontal width of dropdown columns (default: 215px)",
            value: s.min_column_width_px,
            min: 140.0,
            max: 450.0,
            updater: |st, v| st.min_column_width_px = v,
        },
        MetricRowSpec {
            title: "Maximum Submenu Column Height (px)",
            subtitle: "Height before vertical scrolling activates (default: 620px)",
            value: s.max_column_height_px,
            min: 300.0,
            max: 1000.0,
            updater: |st, v| st.max_column_height_px = v,
        },
    ];

    for spec in specs {
        let row = build_action_row(spec.title, spec.subtitle);
        let spin = build_spin_button(spec.value as f64, spec.min, spec.max, 1.0);
        let st = state.clone();
        let updater = spec.updater;
        spin.connect_value_changed(move |sp| {
            updater(
                &mut st.config.borrow_mut().settings.style,
                sp.value_as_int() as u32,
            );
            st.save_and_reload();
        });
        row.add_suffix(&spin);
        metrics_group.add(&row);
    }

    metrics_group
}
