use super::PatchManager;
use super::process_sync::sync_dbus_services_and_kill_stale_daemons;
use crate::applier::StepApplier;
use crate::ledger::PatchLedger;
use crate::models::{ConflictStrategy, PatchManifest, PatchStep};
use genubar_core::{MenuError, PatchErrorPolicy};

pub(crate) struct PatchExecutor;

impl PatchExecutor {
    pub fn apply_patch(
        mgr: &mut PatchManager,
        patch_id: &str,
        strategy: ConflictStrategy,
        global_policy: PatchErrorPolicy,
        skip_unsupported: bool,
    ) -> Result<Option<String>, MenuError> {
        Self::resolve_conflicts_before_apply(mgr, patch_id, strategy)?;

        let manifest = mgr
            .patches
            .iter()
            .find(|p| p.meta.id == patch_id)
            .cloned()
            .ok_or_else(|| MenuError::NotFound(format!("Patch '{patch_id}' not found")))?;

        let effective_policy = manifest.meta.error_policy.unwrap_or(global_policy);
        let mut executed_steps: Vec<PatchStep> = Vec::new();

        for step in &manifest.steps {
            if let PatchStep::Unsupported { raw_type } = step
                && skip_unsupported
            {
                tracing::warn!(raw_type, patch_id, "Skipping unsupported patch step");
                continue;
            }

            let (should_skip, overwrite) =
                Self::evaluate_step_conflict_mode(&mgr.ledger, patch_id, step, strategy);
            if should_skip {
                continue;
            }

            match StepApplier::apply_step(patch_id, step, &mut mgr.ledger, overwrite) {
                Ok(true) => executed_steps.push(step.clone()),
                Ok(false) => {}
                Err(err) => {
                    if Self::should_ignore_step_error(&err, effective_policy) {
                        tracing::warn!(
                            patch_id,
                            policy = ?effective_policy,
                            error = %err,
                            "Ignored patch step error"
                        );
                        continue;
                    }
                    Self::rollback_executed_steps(&mut mgr.ledger, patch_id, &executed_steps);
                    return Err(err);
                }
            }
        }

        sync_dbus_services_and_kill_stale_daemons(&manifest.steps, true);
        mgr.ledger.applied_patches.insert(patch_id.to_string());
        mgr.ledger.save()?;
        Ok(manifest.meta.restart_hint)
    }

    // Multi-app suite patches with `IgnoreSystemErrors` or `IgnoreAll` legitimately skip
    // uninstalled apps during apply, so missing ledger keys for those skipped apps are not corruption.
    pub fn has_corrupted_ledger_entries(mgr: &PatchManager, patch_id: &str) -> bool {
        let Some(manifest) = mgr.patches.iter().find(|p| p.meta.id == patch_id) else {
            return false;
        };
        if matches!(
            manifest.meta.error_policy,
            Some(PatchErrorPolicy::IgnoreSystemErrors | PatchErrorPolicy::IgnoreAll)
        ) {
            return false;
        }
        manifest
            .steps
            .iter()
            .filter_map(StepApplier::resource_key_for_step)
            .any(|(res_key, _)| !mgr.ledger.resources.contains_key(&res_key))
    }

    pub fn revert_patch(
        mgr: &mut PatchManager,
        patch_id: &str,
        force_unsafe: bool,
    ) -> Result<Vec<String>, MenuError> {
        let manifest: PatchManifest = mgr
            .patches
            .iter()
            .find(|p| p.meta.id == patch_id)
            .cloned()
            .ok_or_else(|| MenuError::NotFound(format!("Patch '{patch_id}' not found")))?;

        let lenient_policy = matches!(
            manifest.meta.error_policy,
            Some(PatchErrorPolicy::IgnoreSystemErrors | PatchErrorPolicy::IgnoreAll)
        );

        if !force_unsafe && !lenient_policy && Self::has_corrupted_ledger_entries(mgr, patch_id) {
            return Err(MenuError::CorruptedLedger(patch_id.to_string()));
        }

        let mut warnings = Vec::new();
        for step in manifest.steps.iter().rev() {
            if lenient_policy
                && !force_unsafe
                && let Some((res_key, _)) = StepApplier::resource_key_for_step(step)
                && !mgr.ledger.resources.contains_key(&res_key)
            {
                continue;
            }

            if let Err(err) = StepApplier::revert_step(
                patch_id,
                step,
                &mut mgr.ledger,
                force_unsafe || lenient_policy,
            ) {
                if force_unsafe || lenient_policy {
                    warnings.push(err.to_string());
                } else {
                    return Err(err);
                }
            }
        }

        sync_dbus_services_and_kill_stale_daemons(&manifest.steps, false);
        mgr.ledger.applied_patches.remove(patch_id);
        mgr.ledger.save()?;
        Ok(warnings)
    }

    fn resolve_conflicts_before_apply(
        mgr: &mut PatchManager,
        patch_id: &str,
        strategy: ConflictStrategy,
    ) -> Result<(), MenuError> {
        let conflicts = mgr.check_conflicts(patch_id);
        if conflicts.is_empty() {
            return Ok(());
        }

        match strategy {
            ConflictStrategy::Abort => {
                let owners: Vec<String> = conflicts
                    .into_iter()
                    .flat_map(|c| c.conflicting_owners)
                    .collect();
                Err(MenuError::Config(format!(
                    "Patch '{patch_id}' conflicts with active patches: {owners:?}"
                )))
            }
            ConflictStrategy::DisableConflictingPatch => {
                let owners_to_revert: Vec<String> = conflicts
                    .into_iter()
                    .flat_map(|c| c.conflicting_owners)
                    .collect();
                for old_id in owners_to_revert {
                    Self::revert_patch(mgr, &old_id, false)?;
                }
                Ok(())
            }
            ConflictStrategy::Overwrite | ConflictStrategy::SkipConflictingSteps => Ok(()),
        }
    }

    fn evaluate_step_conflict_mode(
        ledger: &PatchLedger,
        patch_id: &str,
        step: &PatchStep,
        strategy: ConflictStrategy,
    ) -> (bool, bool) {
        let Some((res_key, new_val)) = StepApplier::resource_key_for_step(step) else {
            return (false, false);
        };
        let has_conflict = ledger
            .check_conflict(patch_id, &res_key, &new_val)
            .is_some();

        let should_skip = has_conflict && strategy == ConflictStrategy::SkipConflictingSteps;
        let overwrite = has_conflict && strategy == ConflictStrategy::Overwrite;
        (should_skip, overwrite)
    }

    fn should_ignore_step_error(err: &MenuError, policy: PatchErrorPolicy) -> bool {
        match policy {
            PatchErrorPolicy::Strict => false,
            PatchErrorPolicy::IgnoreSystemErrors => !matches!(
                err,
                MenuError::CheckFailed(_) | MenuError::UnsupportedStep(_)
            ),
            PatchErrorPolicy::IgnoreAll => !matches!(err, MenuError::UnsupportedStep(_)),
        }
    }

    fn rollback_executed_steps(
        ledger: &mut PatchLedger,
        patch_id: &str,
        executed_steps: &[PatchStep],
    ) {
        for prev_step in executed_steps.iter().rev() {
            if let Err(rollback_err) = StepApplier::revert_step(patch_id, prev_step, ledger, false)
            {
                tracing::warn!(
                    patch_id,
                    error = %rollback_err,
                    "Rollback error on patch step"
                );
            }
        }
        if let Err(save_err) = ledger.save() {
            tracing::warn!(error = %save_err, "Failed to save ledger after rollback");
        }
    }
}
