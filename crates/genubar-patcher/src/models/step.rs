use crate::models::condition::{ConditionFailAction, PatchCondition};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::PathBuf;

// Resilient PatchStep enum that captures unknown future step types into `Unsupported`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "type", rename_all = "snake_case")]
pub enum PatchStep {
    Check {
        condition: PatchCondition,
        #[serde(default)]
        on_fail: ConditionFailAction,
        #[serde(default)]
        error_message: Option<String>,
    },
    DesktopOverride {
        desktop_file: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        aliases: Vec<String>,
        #[serde(default)]
        inject_env: BTreeMap<String, String>,
        #[serde(default)]
        append_args: Vec<String>,
        #[serde(default)]
        force_no_dbus_activatable: bool,
    },
    CliWrapper {
        binary_name: String,
        target_binary_path: String,
        #[serde(default)]
        inject_env: BTreeMap<String, String>,
        #[serde(default)]
        append_args: Vec<String>,
    },
    JsonSet {
        file: PathBuf,
        key: String,
        value: serde_json::Value,
    },
    TextLineAppend {
        file_pattern: String,
        line: String,
    },
    FirefoxUserJs {
        pref_key: String,
        pref_value: String,
    },
    FlatpakOverride {
        app_id: String,
        #[serde(default)]
        talk_names: Vec<String>,
        #[serde(default)]
        inject_env: BTreeMap<String, String>,
    },
    GSettingsSet {
        schema: String,
        key: String,
        value: String,
    },
    PythonScript {
        script: PathBuf,
        #[serde(default)]
        args: BTreeMap<String, String>,
    },
    #[serde(skip_deserializing)]
    Unsupported { raw_type: String },
}

impl Serialize for PatchStep {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for PatchStep {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw_val = serde_json::Value::deserialize(deserializer)?;
        if let Ok(known) = Self::deserialize(&raw_val) {
            return Ok(known);
        }

        let raw_type = raw_val
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_step")
            .to_string();
        Ok(Self::Unsupported { raw_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_step_forward_compatible_deserializes_and_preserves_aliases() {
        let step: PatchStep = toml::from_str(
            r#"
            type = "desktop_override"
            desktop_file = "com.microsoft.VSCode.desktop"
            aliases = ["code.desktop"]
            force_no_dbus_activatable = true
            "#,
        )
        .unwrap();
        assert!(matches!(
            step,
            PatchStep::DesktopOverride { ref desktop_file, ref aliases, .. }
            if desktop_file == "com.microsoft.VSCode.desktop" && aliases == &["code.desktop"]
        ));

        let roundtrip = serde_json::to_string(&step).unwrap();
        assert!(roundtrip.contains("\"desktop_override\""));

        let future_step: PatchStep = toml::from_str(
            r#"
            type = "future_systemd_unit"
            unit = "foo.service"
            "#,
        )
        .unwrap();
        assert!(matches!(
            future_step,
            PatchStep::Unsupported { ref raw_type } if raw_type == "future_systemd_unit"
        ));
    }
}
