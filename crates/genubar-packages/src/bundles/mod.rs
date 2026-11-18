pub mod applier;
pub mod inspector;
pub mod manager;

use genubar_config::ConfigManager;
use genubar_core::MenuError;
use genubar_patcher::{PatchManager, PatchManifest};
use std::collections::HashSet;
use std::path::Path;

pub use applier::BundleActionOutcome;
pub use inspector::{BundleComponentItem, BundleDetailedView};
pub use manager::BundleManager;

pub struct BundleOrchestrator;

impl BundleOrchestrator {
    pub(crate) fn matching_patches<'a>(
        patcher: &'a PatchManager,
        bundle_id: &str,
        patch_files: &[String],
    ) -> Vec<&'a PatchManifest> {
        let stems: HashSet<&str> = patch_files
            .iter()
            .map(|f| f.trim_end_matches(".toml"))
            .collect();
        patcher
            .patches
            .iter()
            .filter(|m| {
                m.meta.part_of_bundles.iter().any(|b| b == bundle_id)
                    || stems.contains(m.meta.id.as_str())
            })
            .collect()
    }

    pub fn inspect_bundles(cfg: &ConfigManager, patcher: &PatchManager) -> Vec<BundleDetailedView> {
        inspector::inspect_bundles(cfg, patcher)
    }

    pub fn set_bundle_enabled(
        cfg: &mut ConfigManager,
        patcher: &mut PatchManager,
        bundle_id: &str,
        enabled: bool,
    ) -> Result<BundleActionOutcome, MenuError> {
        applier::set_bundle_enabled(cfg, patcher, bundle_id, enabled)
    }

    pub fn install_from_dir(base_dir: &Path, source_dir: &Path) -> Result<String, MenuError> {
        BundleManager::new(base_dir.to_path_buf()).install_bundle_from_dir(source_dir)
    }

    pub fn uninstall(base_dir: &Path, bundle_id: &str) -> Result<(), MenuError> {
        BundleManager::new(base_dir.to_path_buf()).uninstall_bundle(bundle_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_config::{AppProfile, BundleManifest};
    use genubar_patcher::{PatchLedger, PatchManifest, PackageMetadata};
    use std::fs;

    #[test]
    fn bundle_orchestrator_inspects_live_component_statuses() {
        let tmp_base = std::env::temp_dir().join("genubar_test_bundle_orch");
        let _ = fs::remove_dir_all(&tmp_base);
        let bundle_dir = tmp_base.join("bundles/test_suite");
        fs::create_dir_all(&bundle_dir).unwrap();
        let bm = BundleManifest {
            id: String::from("test_suite"),
            name: String::from("Test Suite"),
            patches: vec![String::from("my_patch.toml")],
            profiles: vec![String::from("nautilus.toml")],
            styles: vec![String::from("sequoia.css")],
            icon_packs: vec![String::from("sf_pack")],
            ..Default::default()
        };
        fs::write(bundle_dir.join("bundle.toml"), toml::to_string(&bm).unwrap()).unwrap();

        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        cfg.profiles.push(AppProfile {
            id: String::from("nautilus"),
            name: String::from("Nautilus"),
            enabled: true,
            ..Default::default()
        });

        let pm = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: vec![PatchManifest {
                meta: PackageMetadata {
                    id: String::from("my_patch"),
                    name: String::from("My Patch"),
                    part_of_bundles: vec![String::from("test_suite")],
                    ..Default::default()
                },
                detection: Default::default(),
                steps: vec![],
            }],
            ledger: PatchLedger::default(),
        };

        let inspected = BundleOrchestrator::inspect_bundles(&cfg, &pm);
        let test_view = inspected
            .iter()
            .find(|v| v.status.manifest.id == "test_suite")
            .expect("test_suite bundle should be present");
        assert_eq!(test_view.components.len(), 4);
        assert_eq!(test_view.active_components, 1);
        let _ = fs::remove_dir_all(&tmp_base);
    }

    #[test]
    fn bundle_orchestrator_installs_toggles_and_uninstalls_local_bundle() {
        let tmp_base = std::env::temp_dir().join("genubar_test_bundle_install_cycle");
        let tmp_src = std::env::temp_dir().join("genubar_test_bundle_src");
        let _ = fs::remove_dir_all(&tmp_base);
        let _ = fs::remove_dir_all(&tmp_src);

        fs::create_dir_all(tmp_src.join("profiles")).unwrap();
        fs::create_dir_all(tmp_src.join("styles")).unwrap();
        fs::create_dir_all(tmp_src.join("icons/sample_icons")).unwrap();
        fs::write(
            tmp_src.join("profiles/editor.toml"),
            "id = \"editor\"\nname = \"Editor\"\nenabled = false\n\n[matcher]\nfield = \"AppId\"\npattern = \"org.example.Editor\"\n",
        )
        .unwrap();
        fs::write(tmp_src.join("styles/theme.css"), "/* theme */\n").unwrap();
        fs::write(
            tmp_src.join("icons/sample_icons/pack.toml"),
            "id = \"sample_icons\"\nname = \"Sample Icons\"\nenabled = false\n",
        )
        .unwrap();

        let bm = BundleManifest {
            id: String::from("local_suite"),
            name: String::from("Local Suite"),
            profiles: vec![String::from("editor.toml")],
            styles: vec![String::from("theme.css")],
            icon_packs: vec![String::from("sample_icons")],
            ..Default::default()
        };
        fs::write(tmp_src.join("bundle.toml"), toml::to_string(&bm).unwrap()).unwrap();

        let installed_id = BundleOrchestrator::install_from_dir(&tmp_base, &tmp_src).unwrap();
        assert_eq!(installed_id, "local_suite");
        assert!(tmp_base.join("bundles/local_suite/bundle.toml").is_file());
        assert!(tmp_base.join("profiles/editor.toml").is_file());
        assert!(tmp_base.join("styles/theme.css").is_file());
        assert!(tmp_base.join("icons/sample_icons/pack.toml").is_file());

        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let mut patcher = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: Vec::new(),
            ledger: PatchLedger::default(),
        };

        let outcome =
            BundleOrchestrator::set_bundle_enabled(&mut cfg, &mut patcher, "local_suite", true)
                .unwrap();
        assert!(outcome.enabled);
        assert_eq!(outcome.bundle_id, "local_suite");
        assert!(cfg.profiles.iter().any(|p| p.id == "editor" && p.enabled));
        assert_eq!(cfg.settings.style.active_style, "theme.css");
        assert!(cfg
            .icon_packs
            .iter()
            .any(|ic| ic.id == "sample_icons" && ic.enabled));

        BundleOrchestrator::uninstall(&tmp_base, "local_suite").unwrap();
        assert!(!tmp_base.join("bundles/local_suite").exists());
        assert!(!tmp_base.join("profiles/editor.toml").exists());
        assert!(!tmp_base.join("styles/theme.css").exists());
        assert!(!tmp_base.join("icons/sample_icons").exists());

        let _ = fs::remove_dir_all(&tmp_base);
        let _ = fs::remove_dir_all(&tmp_src);
    }
}
