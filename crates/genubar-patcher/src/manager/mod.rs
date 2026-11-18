mod execution;
mod process_sync;

use crate::applier::StepApplier;
use crate::detector::SystemDetector;
use crate::ledger::PatchLedger;
use crate::models::{
    ConflictReport, ConflictStrategy, PackageOrigin, PackageStatus, PatchCardView, PatchManifest,
};
use crate::repository::PatchRepository;
use execution::PatchExecutor;
use genubar_core::{ActiveWindowInfo, MenuError, PatchErrorPolicy};
use genubar_platform::fs_utils::read_text_file;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PatchManager {
    pub packages_dir: PathBuf,
    pub patches: Vec<PatchManifest>,
    pub ledger: PatchLedger,
}

impl PatchManager {
    pub fn default_packages_dir() -> PathBuf {
        PatchRepository::default_packages_dir()
    }

    pub fn load_or_init() -> Result<Self, MenuError> {
        Self::load_from_packages_dir(Self::default_packages_dir())
    }

    /// Loads patch manifests from a specific packages directory while sharing the ledger.
    pub fn load_from_packages_dir(packages_dir: PathBuf) -> Result<Self, MenuError> {
        let patches = PatchRepository::load_all_manifests(&packages_dir)?;
        let ledger = PatchLedger::load_or_default();
        Ok(Self {
            packages_dir,
            patches,
            ledger,
        })
    }

    pub fn parse_patch_file_contents(raw_toml: &str) -> Vec<PatchManifest> {
        PatchRepository::parse_patch_file_contents(raw_toml)
    }

    pub fn import_local_patch(&mut self, file_path: &Path) -> Result<Vec<String>, MenuError> {
        let raw = read_text_file(file_path)?;
        let mut parsed = PatchRepository::try_parse_patch_file_contents(&raw).map_err(|err| {
            MenuError::Config(format!(
                "No valid patch manifests in {}: {err}",
                file_path.display()
            ))
        })?;

        let mut imported_ids = Vec::new();
        for mut manifest in parsed.drain(..) {
            manifest.meta.origin = PackageOrigin::ImportedLocal;
            if !manifest.meta.tags.iter().any(|t| t == "imported") {
                manifest.meta.tags.push(String::from("imported"));
            }
            let id = manifest.meta.id.clone();
            PatchRepository::save_manifest_to_dir(&self.packages_dir, &manifest)?;

            self.patches.retain(|p| p.meta.id != id);
            self.patches.push(manifest);
            imported_ids.push(id);
        }

        Ok(imported_ids)
    }

    pub fn list_patch_cards(&self) -> Vec<PatchCardView> {
        self.patches
            .iter()
            .map(|m| {
                let is_applied = self.ledger.applied_patches.contains(&m.meta.id);
                let is_installed_on_host = SystemDetector::is_app_installed(&m.detection);
                let is_compatible = m.is_fully_compatible();
                let incompatibility_reason = m.incompatibility_reason();
                PatchCardView {
                    manifest: m.clone(),
                    status: if is_applied {
                        PackageStatus::Applied
                    } else {
                        PackageStatus::SavedLocal
                    },
                    is_recommended: !is_applied && is_installed_on_host && is_compatible,
                    is_compatible,
                    incompatibility_reason,
                }
            })
            .collect()
    }

    pub fn find_unapplied_recommendation_for_window(
        &self,
        info: &ActiveWindowInfo,
    ) -> Option<&PatchManifest> {
        let matching_patches: Vec<&PatchManifest> = self
            .patches
            .iter()
            .filter(|m| m.is_fully_compatible() && SystemDetector::matches_focused_window(m, info))
            .collect();

        if matching_patches.is_empty()
            || matching_patches
                .iter()
                .any(|m| self.ledger.applied_patches.contains(&m.meta.id))
        {
            return None;
        }

        matching_patches.into_iter().next()
    }

    pub fn check_conflicts(&self, patch_id: &str) -> Vec<ConflictReport> {
        let Some(manifest) = self.patches.iter().find(|p| p.meta.id == patch_id) else {
            return Vec::new();
        };
        manifest
            .steps
            .iter()
            .filter_map(StepApplier::resource_key_for_step)
            .filter_map(|(res_key, new_val)| {
                self.ledger.check_conflict(patch_id, &res_key, &new_val)
            })
            .collect()
    }

    pub fn apply_patch(
        &mut self,
        patch_id: &str,
        strategy: ConflictStrategy,
        global_policy: PatchErrorPolicy,
    ) -> Result<Option<String>, MenuError> {
        self.apply_patch_ext(patch_id, strategy, global_policy, false)
    }

    pub fn apply_patch_ext(
        &mut self,
        patch_id: &str,
        strategy: ConflictStrategy,
        global_policy: PatchErrorPolicy,
        skip_unsupported: bool,
    ) -> Result<Option<String>, MenuError> {
        PatchExecutor::apply_patch(self, patch_id, strategy, global_policy, skip_unsupported)
    }

    pub fn has_corrupted_ledger_entries(&self, patch_id: &str) -> bool {
        PatchExecutor::has_corrupted_ledger_entries(self, patch_id)
    }

    pub fn revert_patch(&mut self, patch_id: &str) -> Result<(), MenuError> {
        let _ = self.revert_patch_ext(patch_id, false)?;
        Ok(())
    }

    pub fn revert_patch_ext(
        &mut self,
        patch_id: &str,
        force_unsafe: bool,
    ) -> Result<Vec<String>, MenuError> {
        PatchExecutor::revert_patch(self, patch_id, force_unsafe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PackageMetadata, PatchStep};

    #[test]
    fn patch_manager_detects_conflicts_and_lenient_ledger_entries() {
        let mut ledger = PatchLedger::default();
        ledger.record_acquire(
            "owner_patch",
            String::from("gsettings:org.gnome.desktop.interface:gtk-enable-primary-paste"),
            Some(String::from("true")),
            String::from("false"),
            false,
        );

        let conflicting = PatchManifest {
            meta: PackageMetadata {
                id: String::from("other_patch"),
                ..Default::default()
            },
            detection: Default::default(),
            steps: vec![PatchStep::GSettingsSet {
                schema: String::from("org.gnome.desktop.interface"),
                key: String::from("gtk-enable-primary-paste"),
                value: String::from("true"),
            }],
        };

        let lenient_suite = PatchManifest {
            meta: PackageMetadata {
                id: String::from("suite_patch"),
                error_policy: Some(PatchErrorPolicy::IgnoreSystemErrors),
                ..Default::default()
            },
            detection: Default::default(),
            steps: vec![PatchStep::GSettingsSet {
                schema: String::from("org.example.app"),
                key: String::from("missing-key"),
                value: String::from("1"),
            }],
        };

        let mgr = PatchManager {
            packages_dir: PathBuf::from("/tmp"),
            patches: vec![conflicting, lenient_suite],
            ledger,
        };

        let conflicts = mgr.check_conflicts("other_patch");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflicting_owners, vec!["owner_patch"]);
        assert!(!mgr.has_corrupted_ledger_entries("suite_patch"));
    }
}
