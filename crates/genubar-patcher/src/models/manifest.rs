use crate::models::package::{PackageKind, PackageMetadata, PackageStatus};
use crate::models::step::PatchStep;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppDetectionRule {
    pub window_matches: Vec<String>,
    pub binaries: Vec<String>,
    pub desktop_files: Vec<String>,
    pub flatpak_ids: Vec<String>,
    pub home_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchManifest {
    pub meta: PackageMetadata,
    #[serde(default)]
    pub detection: AppDetectionRule,
    #[serde(default)]
    pub steps: Vec<PatchStep>,
}

impl PatchManifest {
    pub fn unsupported_step_names(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                PatchStep::Unsupported { raw_type } => Some(raw_type.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn is_fully_compatible(&self) -> bool {
        self.meta.is_compatible_with_current_app() && self.unsupported_step_names().is_empty()
    }

    pub fn incompatibility_reason(&self) -> Option<String> {
        if self.meta.kind == PackageKind::Unknown {
            return Some(String::from("Requires a newer Genubar release (unknown package kind)"));
        }
        if !self.meta.is_compatible_with_current_app() {
            let ver = self.meta.min_app_version.as_deref().unwrap_or("newer");
            return Some(format!("Requires Genubar v{ver}"));
        }
        let unsupported = self.unsupported_step_names();
        if !unsupported.is_empty() {
            return Some(format!(
                "Requires newer Genubar (unsupported step: {})",
                unsupported.join(", ")
            ));
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchCatalogFile {
    #[serde(default)]
    pub patches: Vec<PatchManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchCardView {
    pub manifest: PatchManifest,
    pub status: PackageStatus,
    pub is_recommended: bool,
    pub is_compatible: bool,
    #[serde(default)]
    pub incompatibility_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_patch_with_unsupported_step_parses_as_incompatible() {
        let raw = r#"
            [meta]
            id = "future_v02_patch"
            kind = "patch"
            name = "Future Patch"
            version = "1.2.0"
            min_app_version = "0.2.0"

            [[steps]]
            type = "future_systemd_injection"
            service = "foo.service"
        "#;

        let manifest: PatchManifest = toml::from_str(raw).unwrap();
        assert!(!manifest.is_fully_compatible());
        assert_eq!(
            manifest.unsupported_step_names(),
            vec![String::from("future_systemd_injection")]
        );
        assert_eq!(
            manifest.incompatibility_reason().as_deref(),
            Some("Requires Genubar v0.2.0")
        );

        // Also test when min_app_version is omitted or 0.1.0, but step is unknown
        let step_only: PatchManifest = toml::from_str(
            r#"
            [meta]
            id = "future_step_patch"
            name = "Future Step Only"

            [[steps]]
            type = "brand_new_step"
            arg = "123"
        "#,
        )
        .unwrap();
        assert!(!step_only.is_fully_compatible());
        assert!(
            step_only
                .incompatibility_reason()
                .unwrap()
                .contains("brand_new_step")
        );
    }
}