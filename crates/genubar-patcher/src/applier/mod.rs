pub mod cli_wrapper;
pub mod desktop_override;
pub mod firefox_pref;
pub mod flatpak;
pub mod gsettings;
pub mod json_set;
pub mod python_script;
pub mod text_line;

use crate::condition::{ConditionEvaluator, ConditionFailAction};
use crate::ledger::{PatchLedger, ReleaseOutcome};
use crate::models::PatchStep;
use genubar_core::MenuError;

pub(crate) trait StatefulStepApplier {
    fn resource_key(&self) -> (String, String);
    fn apply_change(&self, patch_id: &str) -> Result<Option<String>, MenuError>;
    fn revert_change(
        &self,
        patch_id: &str,
        orig_opt: Option<String>,
        force_unsafe: bool,
    ) -> Result<(), MenuError>;
}

pub struct StepApplier;

impl StepApplier {
    fn resolve_stateful(step: &PatchStep) -> Option<Box<dyn StatefulStepApplier + '_>> {
        match step {
            PatchStep::Check { .. } | PatchStep::Unsupported { .. } => None,
            PatchStep::DesktopOverride {
                desktop_file,
                aliases,
                inject_env,
                append_args,
                force_no_dbus_activatable,
            } => Some(Box::new(desktop_override::DesktopOverrideApplier {
                desktop_file,
                aliases,
                inject_env,
                append_args,
                force_no_dbus_activatable: *force_no_dbus_activatable,
            })),
            PatchStep::CliWrapper {
                binary_name,
                target_binary_path,
                inject_env,
                append_args,
            } => Some(Box::new(cli_wrapper::CliWrapperApplier {
                binary_name,
                target_binary_path,
                inject_env,
                append_args,
            })),
            PatchStep::JsonSet { file, key, value } => {
                Some(Box::new(json_set::JsonSetApplier { file, key, value }))
            }
            PatchStep::TextLineAppend { file_pattern, line } => {
                Some(Box::new(text_line::TextLineApplier { file_pattern, line }))
            }
            PatchStep::FirefoxUserJs {
                pref_key,
                pref_value,
            } => Some(Box::new(firefox_pref::FirefoxPrefApplier {
                pref_key,
                pref_value,
            })),
            PatchStep::FlatpakOverride {
                app_id,
                talk_names,
                inject_env,
            } => Some(Box::new(flatpak::FlatpakApplier {
                app_id,
                talk_names,
                inject_env,
            })),
            PatchStep::GSettingsSet { schema, key, value } => {
                Some(Box::new(gsettings::GSettingsApplier { schema, key, value }))
            }
            PatchStep::PythonScript { script, args } => {
                Some(Box::new(python_script::PythonScriptApplier { script, args }))
            }
        }
    }

    pub fn resource_key_for_step(step: &PatchStep) -> Option<(String, String)> {
        Self::resolve_stateful(step).map(|handler| handler.resource_key())
    }

    pub fn apply_step(
        patch_id: &str,
        step: &PatchStep,
        ledger: &mut PatchLedger,
        overwrite_owners: bool,
    ) -> Result<bool, MenuError> {
        if let PatchStep::Unsupported { raw_type } = step {
            return Err(MenuError::UnsupportedStep(raw_type.clone()));
        }

        if let PatchStep::Check {
            condition,
            on_fail,
            error_message,
        } = step
        {
            if ConditionEvaluator::evaluate(condition) {
                return Ok(true);
            }
            return match on_fail {
                ConditionFailAction::SkipStep | ConditionFailAction::Unknown => Ok(false),
                ConditionFailAction::AbortPatch => Err(MenuError::CheckFailed(
                    error_message
                        .clone()
                        .unwrap_or_else(|| format!("Precondition failed: {condition:?}")),
                )),
            };
        }

        let handler = Self::resolve_stateful(step)
            .ok_or_else(|| MenuError::Config("Missing resource key for step".into()))?;
        let (res_key, new_val) = handler.resource_key();
        let orig_val = handler.apply_change(patch_id)?;
        ledger.record_acquire(patch_id, res_key, orig_val, new_val, overwrite_owners);
        Ok(true)
    }

    pub fn revert_step(
        patch_id: &str,
        step: &PatchStep,
        ledger: &mut PatchLedger,
        force_unsafe: bool,
    ) -> Result<(), MenuError> {
        let Some(handler) = Self::resolve_stateful(step) else {
            return Ok(());
        };
        let (res_key, _) = handler.resource_key();

        let orig_opt = match ledger.record_release(patch_id, &res_key) {
            ReleaseOutcome::StillOwnedByOthers => return Ok(()),
            ReleaseOutcome::Released(orig) => orig,
            ReleaseOutcome::MissingFromLedger => {
                if !force_unsafe {
                    return Err(MenuError::CorruptedLedger(res_key));
                }
                None
            }
        };

        handler.revert_change(patch_id, orig_opt, force_unsafe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ConditionFailAction, PatchCondition};
    use std::path::PathBuf;

    #[test]
    fn step_applier_computes_keys_and_handles_unsupported_and_checks() {
        let mut ledger = PatchLedger::default();
        let unsupported = PatchStep::Unsupported {
            raw_type: String::from("future_step"),
        };
        assert!(StepApplier::resource_key_for_step(&unsupported).is_none());
        assert!(matches!(
            StepApplier::apply_step("p", &unsupported, &mut ledger, false),
            Err(MenuError::UnsupportedStep(ref s)) if s == "future_step"
        ));

        let skip_check = PatchStep::Check {
            condition: PatchCondition::PathExists {
                path: PathBuf::from("/nonexistent/genubar/check/target"),
            },
            on_fail: ConditionFailAction::SkipStep,
            error_message: None,
        };
        assert!(
            !StepApplier::apply_step("p", &skip_check, &mut ledger, false).unwrap()
        );

        let abort_check = PatchStep::Check {
            condition: PatchCondition::Unknown,
            on_fail: ConditionFailAction::AbortPatch,
            error_message: Some(String::from("custom abort")),
        };
        assert!(matches!(
            StepApplier::apply_step("p", &abort_check, &mut ledger, false),
            Err(MenuError::CheckFailed(ref msg)) if msg == "custom abort"
        ));

        let tmp_json = std::env::temp_dir().join("genubar_test_step_applier_json.json");
        let _ = std::fs::write(&tmp_json, "{\"existing\":1,\"titleBar\":\"custom\"}\n");
        let json_step = PatchStep::JsonSet {
            file: tmp_json.clone(),
            key: String::from("titleBar"),
            value: serde_json::json!("native"),
        };
        assert!(
            StepApplier::apply_step("patch_a", &json_step, &mut ledger, false).unwrap()
        );
        let after_apply_raw = std::fs::read_to_string(&tmp_json).unwrap();
        assert!(after_apply_raw.contains("\"native\""));

        StepApplier::revert_step("patch_a", &json_step, &mut ledger, false).unwrap();
        let after_revert = std::fs::read_to_string(&tmp_json).unwrap();
        assert!(after_revert.contains("\"custom\""));
        let _ = std::fs::remove_file(&tmp_json);

        let desktop_a: PatchStep = toml::from_str(
            r#"
            type = "desktop_override"
            desktop_file = "org.example.App.desktop"
            inject_env = { Z_VAR = "2", A_VAR = "1" }
            "#,
        )
        .unwrap();
        let desktop_b: PatchStep = toml::from_str(
            r#"
            type = "desktop_override"
            desktop_file = "org.example.App.desktop"
            inject_env = { A_VAR = "1", Z_VAR = "2" }
            "#,
        )
        .unwrap();
        assert_eq!(
            StepApplier::resource_key_for_step(&desktop_a),
            StepApplier::resource_key_for_step(&desktop_b)
        );
    }
}
