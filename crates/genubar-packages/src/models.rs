pub use genubar_core::PackageKind;
use genubar_core::{
    CURRENT_APP_VERSION, ExtensionCompatReport, IPC_PROTOCOL_VERSION, is_version_compatible,
};
use genubar_patcher::PatchRepository;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExtensionReleaseManifest {
    pub latest_version: String,
    pub min_daemon_version: Option<String>,
    pub tested_daemon_version: Option<String>,
    pub ipc_protocol_version: Option<u32>,
    pub download_url: Option<String>,
    pub raw_base_url: Option<String>,
    pub changelog: Vec<String>,
}

impl ExtensionReleaseManifest {
    pub fn is_compatible_with_current_daemon(&self) -> bool {
        if let Some(ipc) = self.ipc_protocol_version
            && ipc != IPC_PROTOCOL_VERSION {
                return false;
            }
        if let Some(ref min_d) = self.min_daemon_version
            && !is_version_compatible(min_d, CURRENT_APP_VERSION) {
                return false;
            }
        true
    }
}

pub type InstalledExtensionInfo = ExtensionCompatReport;

#[derive(Debug, Clone)]
pub struct InstalledDaemonInfo {
    pub installed: bool,
    pub version: String,
    pub service_active: bool,
    pub status_summary: String,
}

#[derive(Debug, Clone)]
pub struct AppUpdateInfo {
    pub version: String,
    pub download_url: Option<String>,
    pub changelog: Vec<String>,
    pub extension: Option<ExtensionReleaseManifest>,
}

pub enum UpdateCheckStatus {
    UpToDate {
        extension: Option<ExtensionReleaseManifest>,
    },
    UpdateAvailable(AppUpdateInfo),
    OfflineOrUnreachable,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppReleaseManifest {
    pub latest_app_version: String,
    pub release_date: Option<String>,
    pub download_url: Option<String>,
    pub notes: Option<String>,
    pub changelog: Vec<String>,
    pub extension: Option<ExtensionReleaseManifest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RemoteRegistryCatalog {
    #[serde(alias = "packages", alias = "items")]
    pub entries: Vec<RemotePackageEntry>,
    pub latest_app_version: Option<String>,
    pub download_url: Option<String>,
}

fn default_pkg_version() -> String {
    String::from("1.0.0")
}
fn default_pkg_author() -> String {
    String::from("community")
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RemotePackageEntry {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    #[serde(default = "default_pkg_version")]
    pub version: String,
    #[serde(default = "default_pkg_author")]
    pub author: String,
    pub description: String,
    pub min_app_version: Option<String>,
    pub tags: Vec<String>,
    pub download_url: Option<String>,
    pub content: Option<String>,
    pub files: HashMap<String, String>,
    #[serde(skip)]
    pub source_url: String,
}

impl RemotePackageEntry {
    pub fn version_req_badge(compat: bool, min_app_version: Option<&str>) -> Option<String> {
        (!compat).then(|| {
            format!(
                "Requires Genubar v{}",
                min_app_version.unwrap_or("newer")
            )
        })
    }

    pub fn is_compatible(&self) -> bool {
        if self.kind == PackageKind::Unknown {
            return false;
        }
        if let Some(ref req) = self.min_app_version
            && !is_version_compatible(req, CURRENT_APP_VERSION) {
                return false;
            }
        if self.kind == PackageKind::Patch
            && let Some(ref raw_toml) = self.content {
                let manifests = PatchRepository::parse_patch_file_contents(raw_toml);
                if !manifests.is_empty() && !manifests.iter().all(|m| m.is_fully_compatible()) {
                    return false;
                }
            }
        true
    }

    pub fn incompatibility_badge(&self) -> Option<String> {
        if self.kind == PackageKind::Unknown {
            return Some(String::from("Requires Update (Unknown Package Type)"));
        }
        if let Some(ref req) = self.min_app_version
            && !is_version_compatible(req, CURRENT_APP_VERSION) {
                return Some(format!("Requires Genubar v{req}"));
            }
        if self.kind == PackageKind::Patch
            && let Some(ref raw_toml) = self.content {
                for m in PatchRepository::parse_patch_file_contents(raw_toml) {
                    if let Some(reason) = m.incompatibility_reason() {
                        return Some(reason);
                    }
                }
            }
        None
    }
}

pub enum FetchedPackagePayload {
    TextContent { raw: String, file_name_hint: String },
    BundleFiles { files: HashMap<String, String> },
    LocalBundleDir(PathBuf),
}

#[derive(Debug, Clone)]
pub struct RegistrySourceStatus {
    pub source: String,
    pub loaded_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CatalogSearchItem {
    pub kind: PackageKind,
    pub is_local: bool,
    pub title: String,
    pub subtitle: String,
    pub is_compatible: bool,
    pub incompatibility_badge: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstalledPackageItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub kind: PackageKind,
    pub path: Option<PathBuf>,
    pub can_delete: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_json_with_future_kinds_and_min_app_version_parses_cleanly() {
        let json = r#"{
            "entries": [
                {
                    "id": "gimp_patch",
                    "kind": "patch",
                    "name": "GIMP Patch",
                    "version": "1.0.0",
                    "min_app_version": "0.1.0",
                    "author": "alice",
                    "description": "Works on v0.1.0"
                },
                {
                    "id": "future_ai_widget",
                    "kind": "menubar_widget",
                    "name": "AI Widget",
                    "version": "2.0.0",
                    "min_app_version": "0.2.0",
                    "description": "Future package kind from v0.2"
                },
                {
                    "id": "v02_bundle",
                    "kind": "bundle",
                    "name": "macOS Sequel Pack",
                    "min_app_version": "0.2.0"
                }
            ]
        }"#;

        let catalog: RemoteRegistryCatalog = serde_json::from_str(json).unwrap();
        assert_eq!(catalog.entries.len(), 3);
        assert!(catalog.entries[0].is_compatible());
        assert_eq!(catalog.entries[0].incompatibility_badge(), None);

        assert_eq!(catalog.entries[1].kind, PackageKind::Unknown);
        assert!(!catalog.entries[1].is_compatible());
        assert!(catalog.entries[1].incompatibility_badge().is_some());

        assert!(!catalog.entries[2].is_compatible());
        assert_eq!(
            catalog.entries[2].incompatibility_badge().as_deref(),
            Some("Requires Genubar v0.2.0")
        );

        let embedded: RemoteRegistryCatalog =
            serde_json::from_str(genubar_config::defaults::DEFAULT_EXAMPLE_REGISTRY_JSON).unwrap();
        assert_eq!(embedded.entries.len(), 4);
        assert!(embedded.entries[0].is_compatible());
        assert!(embedded.entries[1].is_compatible());
        assert!(!embedded.entries[2].is_compatible());
        assert_eq!(embedded.entries[3].kind, PackageKind::Unknown);
    }

    #[test]
    fn app_json_release_manifest_and_extension_compat_parse_cleanly() {
        let app_json = r#"{
            "latest_app_version": "0.2.0",
            "release_date": "2026-12-01",
            "download_url": "https://github.com/genubar/genubar/releases/tag/v0.2.0",
            "changelog": ["New widget system", "Faster AT-SPI2"],
            "extension": {
                "latest_version": "0.2.0",
                "min_daemon_version": "0.2.0",
                "ipc_protocol_version": 2,
                "raw_base_url": "https://raw.githubusercontent.com/genubar/genubar-gnome-extension/main"
            }
        }"#;
        let rel: AppReleaseManifest = serde_json::from_str(app_json).unwrap();
        assert!(!is_version_compatible(
            &rel.latest_app_version,
            CURRENT_APP_VERSION
        ));
        assert_eq!(rel.changelog.len(), 2);
        let ext_rel = rel.extension.unwrap();
        assert_eq!(ext_rel.latest_version, "0.2.0");
        assert!(!ext_rel.is_compatible_with_current_daemon());

        let ok_ext = genubar_core::check_extension_metadata_json(Some(
            r#"{"uuid":"genubar@github.com","genubar-version":"0.1.0","min-daemon-version":"0.1.0","ipc-protocol-version":1}"#,
        ));
        assert!(ok_ext.installed);
        assert!(ok_ext.is_compatible);

        let too_new_ext = genubar_core::check_extension_metadata_json(Some(
            r#"{"uuid":"genubar@github.com","genubar-version":"0.2.0","min-daemon-version":"0.2.0","ipc-protocol-version":1}"#,
        ));
        assert!(!too_new_ext.is_compatible);
        assert!(too_new_ext.status_summary.contains("requires Daemon v0.2.0+"));
    }

    #[test]
    fn empty_registry_stub_default_patch_catalog_and_workspace_registry_validate() {
        let empty_stub: RemoteRegistryCatalog = serde_json::from_str(r#"{ "entries": [] }"#).unwrap();
        assert!(empty_stub.entries.is_empty());

        let gnome_patches = genubar_patcher::PatchRepository::try_parse_patch_file_contents(
            genubar_config::defaults::DEFAULT_GNOME_CORE_PATCH_TOML,
        )
        .expect("default gnome_core_apps patch catalog should parse cleanly");
        assert!(!gnome_patches.is_empty());

        let registry_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../genubar-registry");
        if registry_dir.is_dir() {
            let app_raw = std::fs::read_to_string(registry_dir.join("app.json")).unwrap();
            let app_rel: AppReleaseManifest = serde_json::from_str(&app_raw).unwrap();
            assert_eq!(app_rel.latest_app_version, CURRENT_APP_VERSION);
            let ext = app_rel.extension.expect("app.json should publish extension block");
            assert_eq!(ext.ipc_protocol_version, Some(genubar_core::IPC_PROTOCOL_VERSION));
            assert!(ext.is_compatible_with_current_daemon());

            let reg_raw = std::fs::read_to_string(registry_dir.join("registry.json")).unwrap();
            let reg: RemoteRegistryCatalog = serde_json::from_str(&reg_raw).unwrap();
            assert!(reg.entries.is_empty());
        }
    }
}
