use crate::models::{CURRENT_APP_VERSION, is_version_compatible};
use serde::{Deserialize, Serialize};

fn default_version() -> String {
    String::from("1.0.0")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BundleManifest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_app_version: Option<String>,
    pub tags: Vec<String>,
    pub patches: Vec<String>,
    pub profiles: Vec<String>,
    pub styles: Vec<String>,
    pub icon_packs: Vec<String>,
}

impl BundleManifest {
    pub fn is_compatible(&self) -> bool {
        match &self.min_app_version {
            Some(req) => is_version_compatible(req, CURRENT_APP_VERSION),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleStatusView {
    pub manifest: BundleManifest,
    pub total_components: usize,
    pub installed_components: usize,
    #[serde(default)]
    pub is_compatible: bool,
}