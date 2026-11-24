use crate::models::{CURRENT_APP_VERSION, is_version_compatible, menu::MenuNodeDef};
use genubar_core::{ActiveWindowInfo, AtspiSearchConfig, MenuSourceTier, wildcard_match};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}
fn default_author() -> String {
    String::from("genubar")
}
fn default_version() -> String {
    String::from("1.0.0")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    AppId,
    Title,
    Any,
    #[default]
    Wildcard,
    Desktop,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppMatcher {
    pub field: MatchField,
    pub contains: String,
}

impl AppMatcher {
    pub fn matches(&self, info: &ActiveWindowInfo) -> bool {
        let is_desktop = info.pid == 0 && info.xid == 0;
        match self.field {
            MatchField::Unknown => false,
            MatchField::Desktop => is_desktop,
            MatchField::Wildcard => !is_desktop,
            MatchField::AppId | MatchField::Title | MatchField::Any => {
                if is_desktop {
                    return self.contains.eq_ignore_ascii_case("desktop");
                }
                if self.contains.trim().is_empty() {
                    return false;
                }
                let check = |target: &str| -> bool {
                    if self.contains.contains('*') {
                        wildcard_match(&self.contains, target)
                    } else {
                        target
                            .to_lowercase()
                            .contains(&self.contains.to_lowercase())
                    }
                };
                match self.field {
                    MatchField::AppId => check(&info.app_id),
                    MatchField::Title => check(&info.title),
                    MatchField::Any => check(&info.app_id) || check(&info.title),
                    _ => false,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppProfile {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_author")]
    pub author: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub priority: i32,
    pub matchers: Vec<AppMatcher>,
    #[serde(default = "default_true")]
    pub sync_new_items: bool,
    #[serde(default = "default_true")]
    pub include_app_system_column: bool,
    pub fallback_only: bool,
    pub disable_root_sort: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tiers: Option<Vec<MenuSourceTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atspi_override: Option<AtspiSearchConfig>,
    pub volatile_submenus: Vec<String>,
    pub columns: Vec<MenuNodeDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_script: Option<PathBuf>,
    #[serde(skip)]
    pub file_path: PathBuf,
}

impl Default for AppProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: default_version(),
            author: default_author(),
            description: String::new(),
            min_app_version: None,
            enabled: true,
            priority: 0,
            matchers: Vec::new(),
            sync_new_items: true,
            include_app_system_column: true,
            fallback_only: false,
            disable_root_sort: false,
            allowed_tiers: None,
            atspi_override: None,
            volatile_submenus: Vec::new(),
            columns: Vec::new(),
            python_script: None,
            file_path: PathBuf::new(),
        }
    }
}

impl AppProfile {
    pub fn is_compatible(&self) -> bool {
        match &self.min_app_version {
            Some(req) => is_version_compatible(req, CURRENT_APP_VERSION),
            None => true,
        }
    }

    pub fn matches_window(&self, info: &ActiveWindowInfo) -> bool {
        self.enabled && self.is_compatible() && self.matchers.iter().any(|m| m.matches(info))
    }
}
