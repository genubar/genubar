use genubar_core::MenuSourceTier;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

pub(super) fn default_true() -> bool {
    true
}
pub(super) fn default_search_placeholder() -> String {
    String::from("Search menu...")
}
pub(super) fn default_min_windows() -> usize {
    1
}
pub(super) fn default_max_title_len() -> usize {
    48
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DesktopMenuMode {
    #[default]
    Static,
    PythonScript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppTemplateMode {
    #[default]
    Static,
    PythonScript,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopMenuConfig {
    pub mode: DesktopMenuMode,
    pub python_script: Option<PathBuf>,
    pub system_title: Option<String>,
    pub columns: Vec<MenuNodeDef>,
}

impl DesktopMenuConfig {
    /// Returns the configured OS/system title if set and non-empty, otherwise detects the host OS name.
    pub fn resolved_system_title(&self) -> String {
        self.system_title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| genubar_platform::detect_os_name().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppMenuTemplateConfig {
    pub mode: AppTemplateMode,
    #[serde(default = "default_true")]
    pub sync_new_items: bool,
    pub python_script: Option<PathBuf>,
    pub columns: Vec<MenuNodeDef>,
}

impl Default for AppMenuTemplateConfig {
    fn default() -> Self {
        Self {
            mode: AppTemplateMode::Static,
            sync_new_items: true,
            python_script: None,
            columns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "type", rename_all = "snake_case")]
pub enum MenuNodeDef {
    #[serde(alias = "inherit_root")]
    InheritOthers,
    Inherit {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        from: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        match_labels: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        match_aliases: Vec<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_path: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcut: Option<Vec<String>>,
        #[serde(default = "default_true")]
        sync_new_items: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        items: Option<Vec<MenuNodeDef>>,
    },
    Separator {
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_tiers: Option<Vec<MenuSourceTier>>,
    },
    Action {
        #[serde(default)]
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcut: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_tiers: Option<Vec<MenuSourceTier>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        match_aliases: Vec<Vec<String>>,
    },
    Check {
        #[serde(default)]
        label: String,
        #[serde(default)]
        checked: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcut: Option<Vec<String>>,
    },
    Radio {
        #[serde(default)]
        label: String,
        #[serde(default)]
        selected: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcut: Option<Vec<String>>,
    },
    Submenu {
        #[serde(default)]
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_tiers: Option<Vec<MenuSourceTier>>,
        #[serde(default)]
        merge_with_live: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        match_aliases: Vec<Vec<String>>,
        #[serde(default)]
        items: Vec<MenuNodeDef>,
    },
    SearchBar {
        #[serde(default = "default_search_placeholder")]
        placeholder: String,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_tiers: Option<Vec<MenuSourceTier>>,
    },
    WindowList {
        #[serde(default = "default_min_windows")]
        min_windows_to_show: usize,
        #[serde(default = "default_max_title_len")]
        max_title_length: usize,
        #[serde(default = "default_true")]
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_tiers: Option<Vec<MenuSourceTier>>,
    },
    #[serde(skip_deserializing)]
    Unsupported,
}

impl Serialize for MenuNodeDef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for MenuNodeDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw_val = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::deserialize(raw_val).unwrap_or(Self::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        bundle::BundleManifest, profile::AppProfile, settings::GlobalSettings,
    };

    #[test]
    fn forward_compatible_config_models_ignore_unknown_fields_and_future_variants() {
        let profile_toml = r#"
            id = "future_app"
            name = "Future App"
            min_app_version = "99.0.0"
            future_profile_flag = true

            [[matchers]]
            field = "wayland_namespace"
            contains = "org.future"

            [[columns]]
            type = "future_widget_slider"
            min = 0
            max = 100

            [[columns]]
            type = "submenu"
            label = "View"
            items = [
                { type = "future_inline_picker", values = ["a", "b"] },
                { type = "action", label = "Refresh", action = "win.refresh" }
            ]
        "#;
        let profile: AppProfile = toml::from_str(profile_toml).unwrap();
        assert!(!profile.is_compatible());
        assert!(matches!(profile.columns[0], MenuNodeDef::Unsupported));
        assert!(matches!(profile.columns[1], MenuNodeDef::Submenu { .. }));

        let roundtrip = serde_json::to_string(&profile.columns[1]).unwrap();
        assert!(roundtrip.contains("\"type\":\"submenu\""));

        let partial_settings: GlobalSettings = toml::from_str(
            r#"
            [general]
            future_ai_mode = "auto"
            max_top_columns = 12
        "#,
        )
        .unwrap();
        assert_eq!(partial_settings.general.max_top_columns, 12);
        assert!(partial_settings.general.enable_menu_search);
        assert!(partial_settings.general.app_update_url.contains("app.json"));

        let future_bundle: BundleManifest = toml::from_str(
            r#"
            id = "future_bundle"
            name = "v0.2 Pack"
            min_app_version = "0.2.0"
            future_widgets = ["clock"]
        "#,
        )
        .unwrap();
        assert!(!future_bundle.is_compatible());

        let def_settings: GlobalSettings =
            toml::from_str(crate::defaults::DEFAULT_SETTINGS_TOML).unwrap();
        assert!(def_settings.general.app_update_url.ends_with("app.json"));
        let _: DesktopMenuConfig = toml::from_str(crate::defaults::DEFAULT_DESKTOP_TOML).unwrap();
        let _: AppMenuTemplateConfig =
            toml::from_str(crate::defaults::DEFAULT_APP_MENU_TOML).unwrap();
        for prof_toml in [
            crate::defaults::DEFAULT_NAUTILUS_PROFILE_TOML,
            crate::defaults::DEFAULT_PTYXIS_PROFILE_TOML,
            crate::defaults::DEFAULT_TEXT_EDITOR_PROFILE_TOML,
            crate::defaults::DEFAULT_FIREFOX_PROFILE_TOML,
            crate::defaults::DEFAULT_MEDIA_PROFILE_TOML,
            crate::defaults::DEFAULT_LOUPE_PAPERS_PROFILE_TOML,
        ] {
            let p: AppProfile = toml::from_str(prof_toml).unwrap();
            assert!(p.is_compatible());
        }
    }
}
