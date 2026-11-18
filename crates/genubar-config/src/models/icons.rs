use crate::models::{CURRENT_APP_VERSION, is_version_compatible};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_ICON_SIZE_PX: u32 = 12;

fn default_true() -> bool {
    true
}
fn default_icon_size() -> u32 {
    DEFAULT_ICON_SIZE_PX
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColumnReservePolicy {
    Always,
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckPosition {
    #[default]
    BeforeIcon,
    AfterIcon,
    ReplaceIcon,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckVisualStyle {
    #[default]
    Symbol,
    Icon,
    Box,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IconsConfig {
    pub general: IconGeneralConfig,
    pub system_rules: Vec<IconMatchRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconGeneralConfig {
    #[serde(default = "default_true")]
    pub show_menu_icons: bool,
    #[serde(default = "default_true")]
    pub reserve_icon_column: bool,
    #[serde(default)]
    pub icon_column_policy: ColumnReservePolicy,
    #[serde(default)]
    pub check_position: CheckPosition,
    #[serde(default)]
    pub check_column_policy: ColumnReservePolicy,
    #[serde(default)]
    pub check_style: CheckVisualStyle,
    #[serde(default = "default_icon_size")]
    pub icon_size_px: u32,
    #[serde(default = "default_true")]
    pub use_system_fallback: bool,
}

impl Default for IconGeneralConfig {
    fn default() -> Self {
        Self {
            show_menu_icons: true,
            reserve_icon_column: true,
            icon_column_policy: ColumnReservePolicy::default(),
            check_position: CheckPosition::default(),
            check_column_policy: ColumnReservePolicy::default(),
            check_style: CheckVisualStyle::default(),
            icon_size_px: DEFAULT_ICON_SIZE_PX,
            use_system_fallback: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_defaults_match_rust_default_for_icon_general_config() {
        let parsed: IconsConfig = toml::from_str("").unwrap();
        assert_eq!(parsed.general, IconGeneralConfig::default());
        assert_eq!(parsed.general.icon_size_px, 12);
        assert_eq!(parsed.general.icon_column_policy, ColumnReservePolicy::Auto);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IconPackManifest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub priority: i32,
    pub author: String,
    pub description: String,
    pub rules: Vec<IconMatchRule>,
    pub min_app_version: Option<String>,
    #[serde(skip)]
    pub pack_dir: PathBuf,
}

impl IconPackManifest {
    pub fn is_compatible(&self) -> bool {
        match &self.min_app_version {
            Some(req) => is_version_compatible(req, CURRENT_APP_VERSION),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IconPathPattern {
    ArrowString(String),
    Segments(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IconMatchRule {
    pub icon: String,
    #[serde(default)]
    pub match_apps: Vec<String>,
    #[serde(default)]
    pub exclude_apps: Vec<String>,
    #[serde(default)]
    pub match_paths: Vec<IconPathPattern>,
    #[serde(default)]
    pub exclude_paths: Vec<IconPathPattern>,
    #[serde(default)]
    pub match_parent_paths: Vec<IconPathPattern>,
    #[serde(default)]
    pub match_labels: Vec<String>,
    #[serde(default)]
    pub exclude_labels: Vec<String>,
    #[serde(default)]
    pub match_actions: Vec<String>,
    #[serde(default)]
    pub exclude_actions: Vec<String>,
    #[serde(default)]
    pub min_index: Option<usize>,
    #[serde(default)]
    pub max_index: Option<usize>,
    #[serde(default)]
    pub after_label: Option<String>,
    #[serde(default)]
    pub before_label: Option<String>,
}