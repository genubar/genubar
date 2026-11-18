pub use crate::models::condition::{ConditionFailAction, PatchCondition};
use crate::utils::paths::expand_path;
use genubar_platform::desktop_probe::{has_binary_in_path, has_flatpak_app};
use genubar_platform::fs_utils::read_text_file;
use std::env;

pub struct ConditionEvaluator;

impl ConditionEvaluator {
    pub fn evaluate(cond: &PatchCondition) -> bool {
        match cond {
            PatchCondition::BinaryExists { binary } => has_binary_in_path(binary),
            PatchCondition::PathExists { path } => expand_path(path).exists(),
            PatchCondition::FlatpakInstalled { app_id } => has_flatpak_app(app_id),
            PatchCondition::EnvEquals { var, value } => {
                env::var(var).map(|v| v == *value).unwrap_or(false)
            }
            PatchCondition::FileContains { file, substring } => {
                let target = expand_path(file);
                read_text_file(&target)
                    .map(|content| content.contains(substring))
                    .unwrap_or(false)
            }
            PatchCondition::Unknown => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn condition_evaluator_checks_binaries_paths_and_file_contents() {
        assert!(ConditionEvaluator::evaluate(&PatchCondition::BinaryExists {
            binary: String::from("sh"),
        }));
        assert!(!ConditionEvaluator::evaluate(&PatchCondition::BinaryExists {
            binary: String::from("genubar-definitely-nonexistent-bin-12345"),
        }));

        let tmp = std::env::temp_dir().join("genubar_condition_eval_test.txt");
        std::fs::write(&tmp, "enable_menu=true\n").unwrap();

        assert!(ConditionEvaluator::evaluate(&PatchCondition::PathExists {
            path: tmp.clone(),
        }));
        assert!(ConditionEvaluator::evaluate(&PatchCondition::FileContains {
            file: tmp.clone(),
            substring: String::from("enable_menu=true"),
        }));
        assert!(!ConditionEvaluator::evaluate(&PatchCondition::FileContains {
            file: tmp.clone(),
            substring: String::from("missing-token"),
        }));
        let _ = std::fs::remove_file(&tmp);

        assert!(!ConditionEvaluator::evaluate(&PatchCondition::PathExists {
            path: PathBuf::from("/nonexistent/genubar/test/path"),
        }));
        assert!(!ConditionEvaluator::evaluate(&PatchCondition::Unknown));
    }
}