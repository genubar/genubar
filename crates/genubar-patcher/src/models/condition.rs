use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConditionFailAction {
    #[default]
    SkipStep,
    AbortPatch,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "check_type", rename_all = "snake_case")]
pub enum PatchCondition {
    BinaryExists {
        binary: String,
    },
    PathExists {
        path: PathBuf,
    },
    FlatpakInstalled {
        app_id: String,
    },
    EnvEquals {
        var: String,
        value: String,
    },
    FileContains {
        file: PathBuf,
        substring: String,
    },
    #[serde(skip_deserializing)]
    Unknown,
}

impl Serialize for PatchCondition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for PatchCondition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw_val = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::deserialize(raw_val).unwrap_or(Self::Unknown))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_compatible_patch_condition_captures_unknown_variants() {
        let cond: PatchCondition =
            serde_json::from_str(r#"{"check_type":"wayland_compositor","name":"gnome"}"#).unwrap();
        assert!(matches!(cond, PatchCondition::Unknown));

        let known: PatchCondition =
            serde_json::from_str(r#"{"check_type":"binary_exists","binary":"git"}"#).unwrap();
        assert!(matches!(known, PatchCondition::BinaryExists { ref binary } if binary == "git"));
        let roundtrip = serde_json::to_string(&known).unwrap();
        assert!(roundtrip.contains("\"check_type\":\"binary_exists\""));
    }
}
