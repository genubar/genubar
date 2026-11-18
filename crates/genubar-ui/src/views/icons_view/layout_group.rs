use crate::components::controls::build_compact_dropdown;
use crate::components::rows::{build_action_row, build_spin_row, build_switch_row};
use crate::state::AppState;
use adw::prelude::*;
use genubar_config::{CheckPosition, CheckVisualStyle, ColumnReservePolicy};
use libadwaita as adw;

pub fn build_icons_layout_group(state: &AppState) -> adw::PreferencesGroup {
    let gen_group = adw::PreferencesGroup::builder()
        .title("Icons & Checkmarks Layout")
        .description("Configure item icons, two-column alignment, and checkmark placement")
        .build();

    let icons_gen = &state.config.borrow().icons.general;
    let show_menu_icons = icons_gen.show_menu_icons;
    let use_sys_fallback = icons_gen.use_system_fallback;
    let icon_size_px = icons_gen.icon_size_px as f64;
    let icon_col_pol = icons_gen.icon_column_policy;
    let chk_pos = icons_gen.check_position;
    let chk_col_pol = icons_gen.check_column_policy;
    let chk_style = icons_gen.check_style;

    gen_group.add(&build_switch_row(
        state,
        "Show Icons in Menus",
        "Master switch for displaying symbolic and custom icons inside dropdowns",
        show_menu_icons,
        |st, active| {
            st.config.borrow_mut().icons.general.show_menu_icons = active;
        },
    ));

    gen_group.add(&build_switch_row(
        state,
        "Use System Icon Fallback Rules",
        "Assigns Adwaita symbolic icons when no custom icon pack matches an item",
        use_sys_fallback,
        |st, active| {
            st.config.borrow_mut().icons.general.use_system_fallback = active;
        },
    ));

    gen_group.add(&build_spin_row(
        state,
        "Icon Size (px)",
        "Rendered size in pixels (16px is standard Linux grid; 12-14px is compact)",
        icon_size_px,
        10.0,
        28.0,
        1.0,
        |st, val| {
            st.config.borrow_mut().icons.general.icon_size_px = val as u32;
        },
    ));

    let icon_pol_row = build_action_row(
        "Icon Column Reservation",
        "Always reserves a left column so labels align vertically; Never starts text at 0",
    );
    let icon_pol_drop = build_compact_dropdown(
        &["Always", "Auto", "Never"],
        match icon_col_pol {
            ColumnReservePolicy::Always => 0,
            ColumnReservePolicy::Auto => 1,
            ColumnReservePolicy::Never => 2,
        },
    );
    {
        let st = state.clone();
        icon_pol_drop.connect_selected_notify(move |dd| {
            let pol = match dd.selected() {
                1 => ColumnReservePolicy::Auto,
                2 => ColumnReservePolicy::Never,
                _ => ColumnReservePolicy::Always,
            };
            let mut cfg = st.config.borrow_mut();
            cfg.icons.general.icon_column_policy = pol;
            cfg.icons.general.reserve_icon_column = pol != ColumnReservePolicy::Never;
            drop(cfg);
            st.save_and_reload();
        });
    }
    icon_pol_row.add_suffix(&icon_pol_drop);
    gen_group.add(&icon_pol_row);

    let chk_pos_row = build_action_row(
        "Checkmark & Radio Position",
        "Placement of checkmarks relative to the item icon (or on the right before shortcuts)",
    );
    let chk_pos_drop = build_compact_dropdown(
        &["Before Icon", "After Icon", "Share Slot", "Right Side"],
        match chk_pos {
            CheckPosition::BeforeIcon => 0,
            CheckPosition::AfterIcon => 1,
            CheckPosition::ReplaceIcon => 2,
            CheckPosition::Right => 3,
        },
    );
    {
        let st = state.clone();
        chk_pos_drop.connect_selected_notify(move |dd| {
            st.config.borrow_mut().icons.general.check_position = match dd.selected() {
                1 => CheckPosition::AfterIcon,
                2 => CheckPosition::ReplaceIcon,
                3 => CheckPosition::Right,
                _ => CheckPosition::BeforeIcon,
            };
            st.save_and_reload();
        });
    }
    chk_pos_row.add_suffix(&chk_pos_drop);
    gen_group.add(&chk_pos_row);

    let chk_pol_row = build_action_row(
        "Checkmark Column Reservation",
        "Auto reserves space only in submenus that contain at least one Check or Radio item",
    );
    let chk_pol_drop = build_compact_dropdown(
        &["Always", "Auto", "Never"],
        match chk_col_pol {
            ColumnReservePolicy::Always => 0,
            ColumnReservePolicy::Auto => 1,
            ColumnReservePolicy::Never => 2,
        },
    );
    {
        let st = state.clone();
        chk_pol_drop.connect_selected_notify(move |dd| {
            st.config.borrow_mut().icons.general.check_column_policy = match dd.selected() {
                0 => ColumnReservePolicy::Always,
                2 => ColumnReservePolicy::Never,
                _ => ColumnReservePolicy::Auto,
            };
            st.save_and_reload();
        });
    }
    chk_pol_row.add_suffix(&chk_pol_drop);
    gen_group.add(&chk_pol_row);

    let chk_style_row = build_action_row(
        "Checkmark Visual Style",
        "Choose between clean symbols (✓ / •), system icons, or framed boxes (☑ / ☐)",
    );
    let chk_style_drop = build_compact_dropdown(
        &["Symbol", "System Icon", "Framed Box"],
        match chk_style {
            CheckVisualStyle::Symbol => 0,
            CheckVisualStyle::Icon => 1,
            CheckVisualStyle::Box => 2,
        },
    );
    {
        let st = state.clone();
        chk_style_drop.connect_selected_notify(move |dd| {
            st.config.borrow_mut().icons.general.check_style = match dd.selected() {
                1 => CheckVisualStyle::Icon,
                2 => CheckVisualStyle::Box,
                _ => CheckVisualStyle::Symbol,
            };
            st.save_and_reload();
        });
    }
    chk_style_row.add_suffix(&chk_style_drop);
    gen_group.add(&chk_style_row);

    gen_group
}
