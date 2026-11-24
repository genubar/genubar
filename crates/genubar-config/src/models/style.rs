use crate::models::{CURRENT_APP_VERSION, is_version_compatible};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleMetadata {
    pub file_name: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_app_version: Option<String>,
}

impl StyleMetadata {
    pub fn is_compatible(&self) -> bool {
        match &self.min_app_version {
            Some(req) => is_version_compatible(req, CURRENT_APP_VERSION),
            None => true,
        }
    }
}
