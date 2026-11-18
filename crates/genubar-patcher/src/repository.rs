use crate::models::{PatchCatalogFile, PatchManifest};
use genubar_core::MenuError;
use genubar_platform::config_dir;
use genubar_platform::fs_utils::{read_text_file, write_text_file};
use std::fs;
use std::path::{Path, PathBuf};

const BUILTIN_PATCHES_TOML: &str = include_str!("../defaults/builtin_patches.toml");

pub struct PatchRepository;

impl PatchRepository {
    pub fn default_packages_dir() -> PathBuf {
        config_dir().join("patches")
    }

    /// Loads built-in patches and overlays user `.toml` manifests from `packages_dir`.
    pub fn load_all_manifests(packages_dir: &Path) -> Result<Vec<PatchManifest>, MenuError> {
        fs::create_dir_all(packages_dir).map_err(|e| {
            MenuError::Config(format!(
                "Failed to create patches directory {}: {e}",
                packages_dir.display()
            ))
        })?;

        let mut patches = Vec::new();
        let builtin = toml::from_str::<PatchCatalogFile>(BUILTIN_PATCHES_TOML).map_err(|e| {
            MenuError::ParseError(format!("Failed to parse builtin_patches.toml: {e}"))
        })?;
        patches.extend(builtin.patches);

        let entries = fs::read_dir(packages_dir).map_err(|e| {
            MenuError::Config(format!(
                "Failed to read directory {}: {e}",
                packages_dir.display()
            ))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                match read_text_file(&path) {
                    Ok(raw) => match Self::try_parse_patch_file_contents(&raw) {
                        Ok(parsed) => {
                            for loaded in parsed {
                                patches.retain(|existing| existing.meta.id != loaded.meta.id);
                                patches.push(loaded);
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                error = %err,
                                path = %path.display(),
                                "Failed parsing patch file"
                            );
                        }
                    },
                    Err(err) => {
                        tracing::warn!(error = %err, path = %path.display(), "Failed to read patch file");
                    }
                }
            }
        }

        Ok(patches)
    }

    /// Parses either a multi-patch catalog (`[[patches]]`) or a single `PatchManifest`, returning TOML parse errors.
    pub fn try_parse_patch_file_contents(raw_toml: &str) -> Result<Vec<PatchManifest>, MenuError> {
        let catalog_err = match toml::from_str::<PatchCatalogFile>(raw_toml) {
            Ok(catalog) => {
                if !catalog.patches.is_empty() {
                    return Ok(catalog.patches);
                }
                None
            }
            Err(err) => Some(err),
        };
        match toml::from_str::<PatchManifest>(raw_toml) {
            Ok(single) => Ok(vec![single]),
            Err(single_err) => {
                let details = match catalog_err {
                    Some(cat_err) => {
                        format!("Invalid patch TOML (catalog: {cat_err}; single: {single_err})")
                    }
                    None => format!("Invalid patch TOML: {single_err}"),
                };
                Err(MenuError::ParseError(details))
            }
        }
    }

    /// Best-effort wrapper around [`Self::try_parse_patch_file_contents`] that logs parse errors and returns `Vec::new()`.
    pub fn parse_patch_file_contents(raw_toml: &str) -> Vec<PatchManifest> {
        match Self::try_parse_patch_file_contents(raw_toml) {
            Ok(patches) => patches,
            Err(err) => {
                tracing::warn!(error = %err, "Failed parsing patch TOML contents");
                Vec::new()
            }
        }
    }

    pub fn save_manifest_to_dir(
        packages_dir: &Path,
        manifest: &PatchManifest,
    ) -> Result<PathBuf, MenuError> {
        let dest = packages_dir.join(format!("{}.toml", manifest.meta.id));
        let serialized = toml::to_string_pretty(manifest).map_err(|e| {
            MenuError::Config(format!(
                "Failed to serialize patch '{}': {e}",
                manifest.meta.id
            ))
        })?;
        write_text_file(&dest, &serialized)?;
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applier::StepApplier;
    use crate::ledger::PatchLedger;
    use crate::models::PatchStep;

    #[test]
    fn try_parse_patch_file_contents_reports_toml_parse_errors() {
        let err = PatchRepository::try_parse_patch_file_contents("[meta\nbroken = true").unwrap_err();
        assert!(matches!(err, MenuError::ParseError(_)));
        assert!(err.to_string().contains("Invalid patch TOML"));
    }

    #[test]
    fn builtin_patches_catalog_deserializes_with_valid_manifests_and_steps() {
        let tmp = std::env::temp_dir().join("genubar_test_builtin_patches");
        let _ = fs::remove_dir_all(&tmp);
        let patches = PatchRepository::load_all_manifests(&tmp).unwrap();
        assert!(!patches.is_empty());
        assert!(patches.iter().all(|p| !p.meta.id.is_empty() && !p.steps.is_empty()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn step_applier_and_ledger_handle_shared_ownership_and_clean_rollback() {
        let tmp = std::env::temp_dir().join("genubar_test_patch_apply_revert");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let json_path = tmp.join("settings.json");
        fs::write(&json_path, "{\n  \"window.menuBarVisibility\": \"toggle\"\n}\n").unwrap();

        let text_path = tmp.join("env.conf");
        fs::write(&text_path, "EXISTING=1\n").unwrap();

        let json_step = PatchStep::JsonSet {
            file: json_path.clone(),
            key: String::from("window.menuBarVisibility"),
            value: serde_json::json!("classic"),
        };
        let text_step = PatchStep::TextLineAppend {
            file_pattern: text_path.to_string_lossy().to_string(),
            line: String::from("GTK_USE_PORTAL=1"),
        };

        let mut ledger = PatchLedger::default();
        assert!(StepApplier::apply_step("patch_a", &json_step, &mut ledger, false).unwrap());
        assert!(StepApplier::apply_step("patch_a", &text_step, &mut ledger, false).unwrap());

        // Second owner acquiring identical JSON setting shares ownership without overwriting original_value.
        assert!(StepApplier::apply_step("patch_b", &json_step, &mut ledger, false).unwrap());

        let (res_key, _) = StepApplier::resource_key_for_step(&json_step).unwrap();
        assert!(ledger.check_conflict("patch_c", &res_key, "\"hidden\"").is_some());
        assert!(ledger.check_conflict("patch_c", &res_key, "\"classic\"").is_none());

        let mid_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(mid_json["window.menuBarVisibility"], "classic");
        assert!(fs::read_to_string(&text_path).unwrap().contains("GTK_USE_PORTAL=1"));

        // Reverting patch_a must keep JSON at "classic" while patch_b still owns it, and revert text_step.
        StepApplier::revert_step("patch_a", &json_step, &mut ledger, false).unwrap();
        StepApplier::revert_step("patch_a", &text_step, &mut ledger, false).unwrap();
        let after_a: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(after_a["window.menuBarVisibility"], "classic");
        assert_eq!(fs::read_to_string(&text_path).unwrap(), "EXISTING=1\n");

        // Reverting last owner (patch_b) restores the original "toggle" value.
        StepApplier::revert_step("patch_b", &json_step, &mut ledger, false).unwrap();
        let final_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(final_json["window.menuBarVisibility"], "toggle");
        assert!(ledger.resources.is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }
}