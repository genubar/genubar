pub use genubar_core::PackageKind;
use genubar_core::{CURRENT_APP_VERSION, PatchErrorPolicy, is_version_compatible};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn default_version() -> String {
    String::from("1.0.0")
}

fn default_author() -> String {
    String::from("community")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum PackageOrigin {
    #[default]
    BuiltIn,
    ImportedLocal,
    DownloadedRemote {
        #[serde(default)]
        registry_url: String,
    },
    #[serde(skip_deserializing)]
    Unknown,
}

impl Serialize for PackageOrigin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for PackageOrigin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw_val = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::deserialize(raw_val).unwrap_or(Self::Unknown))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageStatus {
    RemoteAvailable,
    SavedLocal,
    Applied,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PackageMetadata {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_author")]
    pub author: String,
    pub description: String,
    pub min_app_version: Option<String>,
    pub tags: Vec<String>,
    pub part_of_bundles: Vec<String>,
    pub restart_hint: Option<String>,
    pub error_policy: Option<PatchErrorPolicy>,
    pub origin: PackageOrigin,
}

impl PackageMetadata {
    pub fn is_compatible_with_current_app(&self) -> bool {
        if self.kind == PackageKind::Unknown {
            return false;
        }
        match &self.min_app_version {
            Some(req) => is_version_compatible(req, CURRENT_APP_VERSION),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_compatible_package_kind_and_origin_deserialize_safely() {
        let raw = r#"
            id = "future_widget_pack"
            kind = "desktop_widget"
            name = "Future Widget"
            origin = "cloud_sync"
            min_app_version = "0.1.0"
            some_future_field = 42
        "#;
        let meta: PackageMetadata = toml::from_str(raw).unwrap();
        assert_eq!(meta.kind, PackageKind::Unknown);
        assert_eq!(meta.origin, PackageOrigin::Unknown);
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.author, "community");
        assert!(!meta.is_compatible_with_current_app());
    }

    #[test]
    fn min_app_version_blocks_incompatible_and_allows_compatible() {
        let future_pkg: PackageMetadata = toml::from_str(
            r#"
            id = "future_patch"
            kind = "patch"
            min_app_version = "99.0.0"
        "#,
        )
        .unwrap();
        assert!(!future_pkg.is_compatible_with_current_app());

        let cur_pkg: PackageMetadata = toml::from_str(
            r#"
            id = "cur_patch"
            kind = "patch"
            min_app_version = "0.1.0"
        "#,
        )
        .unwrap();
        assert!(cur_pkg.is_compatible_with_current_app());
    }
}