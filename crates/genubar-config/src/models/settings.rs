use genubar_core::{AtspiSearchConfig, PatchErrorPolicy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_app_update_url() -> String {
    String::from("https://raw.githubusercontent.com/genubar/genubar-registry/main/app.json")
}
fn default_registry_urls() -> Vec<String> {
    vec![String::from(
        "https://raw.githubusercontent.com/genubar/genubar-registry/main/registry.json",
    )]
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub general: GeneralConfig,
    pub root_sorting: RootSortConfig,
    pub style: StyleFileConfig,
    pub api_tiers: ApiTiersConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    #[serde(default = "default_app_update_url")]
    pub app_update_url: String,
    #[serde(default = "default_registry_urls")]
    pub registry_urls: Vec<String>,
    pub python_binary: String,
    pub script_timeout_ms: u64,
    pub enable_more_overflow: bool,
    pub max_top_columns: usize,
    pub more_column_label: String,
    pub patch_error_policy: PatchErrorPolicy,
    pub enable_menu_search: bool,
    pub show_menu_icons: bool,
    pub auto_map_standard_icons: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            app_update_url: default_app_update_url(),
            registry_urls: default_registry_urls(),
            python_binary: String::from("python3"),
            script_timeout_ms: 500,
            enable_more_overflow: false,
            max_top_columns: 10,
            more_column_label: String::from("More"),
            patch_error_policy: PatchErrorPolicy::Strict,
            enable_menu_search: true,
            show_menu_icons: true,
            auto_map_standard_icons: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RootSortMode {
    Disabled,
    #[default]
    Standard,
    Custom,
    PythonScript,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RootSortConfig {
    pub mode: RootSortMode,
    pub custom_order: Vec<String>,
    pub exclude_apps: Vec<String>,
    pub python_script: Option<PathBuf>,
}

impl Default for RootSortConfig {
    fn default() -> Self {
        Self {
            mode: RootSortMode::Standard,
            custom_order: vec![
                "{app_name}".to_string(),
                "File".to_string(),
                "Edit".to_string(),
                "Selection".to_string(),
                "View".to_string(),
                "Go".to_string(),
                "*".to_string(),
                "Tools".to_string(),
                "Window".to_string(),
                "Help".to_string(),
            ],
            exclude_apps: Vec::new(),
            python_script: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CssSyncMode {
    SymlinkToExtension,
    #[default]
    CopyToExtension,
    DirectExtensionFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleFileConfig {
    pub sync_mode: CssSyncMode,
    pub custom_css_path: PathBuf,
    pub extension_css_path: PathBuf,
    pub active_style: String,
    pub theme_mode: ThemeMode,
    pub recolor_gnome_panel: bool,
    pub apply_metrics_to_css: bool,
    pub max_column_height_px: u32,
    pub min_column_width_px: u32,
    pub panel_button_hpadding_px: u32,
    pub item_vpadding_px: u32,
    pub item_hpadding_px: u32,
    pub popup_border_radius_px: u32,
    pub item_border_radius_px: u32,
    pub font_size_px: u32,
}

impl Default for StyleFileConfig {
    fn default() -> Self {
        Self {
            sync_mode: CssSyncMode::CopyToExtension,
            custom_css_path: PathBuf::from("style.css"),
            extension_css_path: PathBuf::from(
                "~/.local/share/gnome-shell/extensions/genubar@github.com/stylesheet.css",
            ),
            active_style: String::from("default.css"),
            theme_mode: ThemeMode::Auto,
            recolor_gnome_panel: true,
            apply_metrics_to_css: true,
            max_column_height_px: 620,
            min_column_width_px: 215,
            panel_button_hpadding_px: 6,
            item_vpadding_px: 5,
            item_hpadding_px: 10,
            popup_border_radius_px: 14,
            item_border_radius_px: 9,
            font_size_px: 13,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiTiersConfig {
    pub gtk4_hook: Gtk4HookTierConfig,
    pub gtk_native: GtkNativeTierConfig,
    pub canonical_dbus: CanonicalDbusTierConfig,
    pub atspi: AtspiTierConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GtkNativeTierConfig {
    pub enabled: bool,
    pub include_app_menu_column: bool,
    pub python_script: Option<PathBuf>,
}

impl Default for GtkNativeTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            include_app_menu_column: true,
            python_script: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Gtk4LayoutAlgorithm {
    Original,
    #[default]
    Standard,
    PythonScript,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Gtk4HookTierConfig {
    pub enabled: bool,
    pub layout_algorithm: Gtk4LayoutAlgorithm,
    pub expose_unknown_custom_widgets: bool,
    pub hide_original_buttons: bool,
    pub custom_hook_so_path: Option<PathBuf>,
    pub python_script: Option<PathBuf>,
}

impl Default for Gtk4HookTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            layout_algorithm: Gtk4LayoutAlgorithm::Standard,
            expose_unknown_custom_widgets: true,
            hide_original_buttons: false,
            custom_hook_so_path: None,
            python_script: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CanonicalDbusTierConfig {
    pub enabled: bool,
    pub preload_passes: u8,
    pub swing_open_delay_ms: u64,
    pub cache_last_valid_menu: bool,
    pub python_script: Option<PathBuf>,
}

impl Default for CanonicalDbusTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preload_passes: 2,
            swing_open_delay_ms: 35,
            cache_last_valid_menu: true,
            python_script: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtspiTierConfig {
    pub enabled: bool,
    pub enable_disk_cache: bool,
    pub append_refresh_cache_button: bool,
    pub default_volatile_submenus: Vec<String>,
    pub search: AtspiSearchConfig,
    pub python_script: Option<PathBuf>,
}

impl Default for AtspiTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            enable_disk_cache: true,
            append_refresh_cache_button: true,
            default_volatile_submenus: Vec::new(),
            search: AtspiSearchConfig {
                target_roles: vec![String::from("menu bar")],
                name_contains: None,
                traverse_document: false,
                hoist_single_button: false,
                max_depth: 6,
                max_nodes_budget: 85,
            },
            python_script: None,
        }
    }
}