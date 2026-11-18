use super::StatefulStepApplier;
use genubar_core::MenuError;
use std::process::Command;

pub(crate) struct GSettingsApplier<'a> {
    pub schema: &'a str,
    pub key: &'a str,
    pub value: &'a str,
}

impl StatefulStepApplier for GSettingsApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("gsettings:{}:{}", self.schema, self.key),
            self.value.to_string(),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(self.schema, self.key, self.value)
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        orig_opt: Option<String>,
        force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.schema, self.key, orig_opt, force_unsafe)
    }
}

pub fn apply(schema: &str, key: &str, value: &str) -> Result<Option<String>, MenuError> {
    let prev_val = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let output = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output()
        .map_err(|e| MenuError::IoPermission(format!("Failed to spawn gsettings: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MenuError::IoPermission(format!(
            "gsettings set {schema} {key} failed: {}",
            stderr.trim()
        )));
    }

    Ok(prev_val)
}

pub fn revert(
    schema: &str,
    key: &str,
    orig_opt: Option<String>,
    force_unsafe: bool,
) -> Result<(), MenuError> {
    let output = match orig_opt {
        Some(prev) => Command::new("gsettings")
            .args(["set", schema, key, &prev])
            .output(),
        None if force_unsafe => Command::new("gsettings")
            .args(["reset", schema, key])
            .output(),
        None => return Ok(()),
    }
        .map_err(|e| MenuError::IoPermission(format!("Failed to execute gsettings revert: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MenuError::IoPermission(format!(
            "gsettings revert failed for {schema} {key}: {}",
            stderr.trim()
        )));
    }
    Ok(())
}