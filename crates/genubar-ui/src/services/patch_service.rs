use crate::error::UiError;
use genubar_config::ConfigManager;
use genubar_core::{MenuError, PatchErrorPolicy};
use genubar_patcher::{ConflictStrategy, PatchManager};
use std::path::Path;

pub struct PatchService;

/// Result of a patch mutation executed on a background worker thread.
pub struct PatchOpOutcome {
    pub patcher: PatchManager,
    pub message: String,
    pub needs_reload: bool,
}

impl PatchService {
    pub fn apply_skipping_unsupported(
        patcher: &mut PatchManager,
        patch_id: &str,
        strategy: ConflictStrategy,
        policy: PatchErrorPolicy,
    ) -> Result<String, UiError> {
        patcher
            .apply_patch_ext(patch_id, strategy, policy, true)
            .map(|opt_hint| match opt_hint {
                Some(hint) => format!("Force-applied '{patch_id}'. {hint}"),
                None => format!("Force-applied '{patch_id}' (skipped unsupported steps)"),
            })
            .map_err(UiError::Domain)
    }

    /// Orchestrates patch application with automatic fallback for unsupported future step types.
    pub fn apply_with_fallback(
        patcher: &mut PatchManager,
        patch_id: &str,
        strategy: ConflictStrategy,
        policy: PatchErrorPolicy,
    ) -> Result<String, UiError> {
        match patcher.apply_patch_ext(patch_id, strategy, policy, false) {
            Ok(Some(hint)) => Ok(format!("Applied '{patch_id}'. {hint}")),
            Ok(None) => Ok(format!("Applied patch '{patch_id}'")),
            Err(MenuError::UnsupportedStep(step_name)) => patcher
                .apply_patch_ext(patch_id, strategy, policy, true)
                .map(|_| {
                    format!("Applied '{patch_id}' (skipped unsupported command '{step_name}')")
                })
                .map_err(UiError::Domain),
            Err(e) => Err(UiError::Domain(e)),
        }
    }

    /// Orchestrates patch rollback with automatic best-effort cleanup if `patch_ledger.json` is incomplete.
    pub fn revert_with_fallback(
        patcher: &mut PatchManager,
        patch_id: &str,
    ) -> Result<String, UiError> {
        match patcher.revert_patch_ext(patch_id, false) {
            Ok(_) => Ok(format!("Reverted patch '{patch_id}'")),
            Err(MenuError::CorruptedLedger(_)) => patcher
                .revert_patch_ext(patch_id, true)
                .map(|warns| {
                    format!(
                        "Ledger incomplete; forced cleanup finished for '{patch_id}' ({} warnings)",
                        warns.len()
                    )
                })
                .map_err(UiError::Domain),
            Err(e) => Err(UiError::Domain(e)),
        }
    }

    /// Reverts (best-effort) if applied, deletes the patch TOML file, and drops it from `patcher.patches`.
    pub fn delete_patch_file(
        patcher: &mut PatchManager,
        patch_id: &str,
        patch_file: &Path,
    ) -> Result<bool, UiError> {
        let was_applied = patcher.ledger.applied_patches.contains(patch_id);
        if was_applied && let Err(err) = patcher.revert_patch_ext(patch_id, true) {
            tracing::warn!(
                patch_id = %patch_id,
                error = %err,
                "Rollback encountered errors prior to deleting patch file"
            );
        }
        if patch_file.is_file() {
            std::fs::remove_file(patch_file).map_err(|source| UiError::Io {
                context: format!("delete patch file {}", patch_file.display()),
                source,
            })?;
        }
        patcher.patches.retain(|p| p.meta.id != patch_id);
        Ok(was_applied)
    }

    /// Reverts all currently applied patches.
    pub fn revert_all_applied_patches(patcher: &mut PatchManager) -> usize {
        let applied_ids: Vec<String> = patcher.ledger.applied_patches.iter().cloned().collect();
        let mut count = 0;
        for id in applied_ids {
            if Self::revert_with_fallback(patcher, &id).is_ok() {
                count += 1;
            }
        }
        count
    }

    pub fn apply_owned(
        mut patcher: PatchManager,
        patch_id: String,
        strategy: ConflictStrategy,
        policy: PatchErrorPolicy,
        skip_unsupported: bool,
    ) -> Result<PatchOpOutcome, UiError> {
        let message = if skip_unsupported {
            Self::apply_skipping_unsupported(&mut patcher, &patch_id, strategy, policy)?
        } else {
            Self::apply_with_fallback(&mut patcher, &patch_id, strategy, policy)?
        };
        Ok(PatchOpOutcome {
            patcher,
            message,
            needs_reload: true,
        })
    }

    pub fn revert_owned(
        mut patcher: PatchManager,
        patch_id: String,
    ) -> Result<PatchOpOutcome, UiError> {
        let message = Self::revert_with_fallback(&mut patcher, &patch_id)?;
        Ok(PatchOpOutcome {
            patcher,
            message,
            needs_reload: true,
        })
    }

    pub fn delete_owned(
        mut patcher: PatchManager,
        patch_id: String,
        patch_file: std::path::PathBuf,
    ) -> Result<PatchOpOutcome, UiError> {
        let was_applied = Self::delete_patch_file(&mut patcher, &patch_id, &patch_file)?;
        Ok(PatchOpOutcome {
            patcher,
            message: format!("Deleted patch '{patch_id}'"),
            needs_reload: was_applied,
        })
    }

    pub fn revert_all_owned(mut patcher: PatchManager) -> PatchOpOutcome {
        let count = Self::revert_all_applied_patches(&mut patcher);
        PatchOpOutcome {
            patcher,
            message: format!("Reverted {count} applied patch(es)"),
            needs_reload: count > 0,
        }
    }

    pub fn reload_patcher_owned() -> Result<PatchManager, UiError> {
        PatchManager::load_or_init().map_err(UiError::Domain)
    }

    pub fn patch_file_path(cfg: &ConfigManager, patch_id: &str) -> std::path::PathBuf {
        cfg.base_dir()
            .join("patches")
            .join(format!("{patch_id}.toml"))
    }
}
