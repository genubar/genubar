use genubar_config::{CheckPosition, CheckVisualStyle, ColumnReservePolicy, ThemeMode};
use genubar_core::MenuNode;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UiRenderConfig {
    pub show_menu_icons: bool,
    pub reserve_icon_column: bool,
    pub icon_column_policy: ColumnReservePolicy,
    pub check_position: CheckPosition,
    pub check_column_policy: ColumnReservePolicy,
    pub check_style: CheckVisualStyle,
    pub icon_size_px: u32,
    pub theme_mode: ThemeMode,
    pub recolor_gnome_panel: bool,
    pub css_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuPayload {
    pub app_title: String,
    pub app_id: String,
    pub pid: u32,
    pub source: String,
    pub ui_config: UiRenderConfig,
    pub root: MenuNode,
}
