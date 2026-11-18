use crate::error::UiError;
use genubar_config::ConfigManager;
use genubar_packages::{BundleDetailedView, BundleOrchestrator};
use genubar_patcher::PatchManager;
use std::path::PathBuf;

pub struct BundleService;

/// Result of a bundle install/enable/disable/remove operation executed off the GTK main thread.
pub struct BundleOpOutcome {
    pub config: ConfigManager,
    pub patcher: PatchManager,
    pub message: String,
    pub needs_reload: bool,
}

impl BundleService {
    pub fn inspect_bundles(cfg: &ConfigManager, patcher: &PatchManager) -> Vec<BundleDetailedView> {
        BundleOrchestrator::inspect_bundles(cfg, patcher)
    }

    pub fn set_bundle_enabled_owned(
        mut cfg: ConfigManager,
        mut patcher: PatchManager,
        bundle_id: String,
        enabled: bool,
    ) -> Result<BundleOpOutcome, UiError> {
        let outcome =
            BundleOrchestrator::set_bundle_enabled(&mut cfg, &mut patcher, &bundle_id, enabled)?;

        let message = if enabled {
            if outcome.changed_patches.is_empty() {
                format!(
                    "Enabled bundle '{}' (profiles, style & icons active)",
                    outcome.bundle_id
                )
            } else {
                format!(
                    "Enabled bundle '{}' and applied {} patch(es)",
                    outcome.bundle_id,
                    outcome.changed_patches.len()
                )
            }
        } else {
            format!(
                "Disabled bundle '{}' and reverted {} patch(es)",
                outcome.bundle_id,
                outcome.changed_patches.len()
            )
        };

        Ok(BundleOpOutcome {
            config: cfg,
            patcher,
            message,
            needs_reload: true,
        })
    }

    pub fn remove_bundle_owned(
        mut cfg: ConfigManager,
        mut patcher: PatchManager,
        bundle_id: String,
    ) -> Result<BundleOpOutcome, UiError> {
        if let Err(err) =
            BundleOrchestrator::set_bundle_enabled(&mut cfg, &mut patcher, &bundle_id, false)
        {
            tracing::warn!(
                bundle_id = %bundle_id,
                error = %err,
                "Failed reverting bundle components cleanly prior to uninstall"
            );
        }
        let base_dir = cfg.base_dir().to_path_buf();
        let packages_dir = patcher.packages_dir.clone();
        BundleOrchestrator::uninstall(&base_dir, &bundle_id)?;
        let fresh_cfg = ConfigManager::load_from_base_dir(base_dir)?;
        let fresh_patcher = PatchManager::load_from_packages_dir(packages_dir)?;

        Ok(BundleOpOutcome {
            config: fresh_cfg,
            patcher: fresh_patcher,
            message: format!("Removed bundle '{bundle_id}'"),
            needs_reload: true,
        })
    }

    pub fn install_from_dir_owned(
        base_dir: PathBuf,
        source_dir: PathBuf,
    ) -> Result<BundleOpOutcome, UiError> {
        let bundle_id = BundleOrchestrator::install_from_dir(&base_dir, &source_dir)?;
        let packages_dir = base_dir.join("patches");
        let fresh_cfg = ConfigManager::load_from_base_dir(base_dir)?;
        let fresh_patcher = PatchManager::load_from_packages_dir(packages_dir)?;

        Ok(BundleOpOutcome {
            config: fresh_cfg,
            patcher: fresh_patcher,
            message: format!("Installed bundle '{bundle_id}'"),
            needs_reload: true,
        })
    }
}
