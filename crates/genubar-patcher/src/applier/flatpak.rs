use super::StatefulStepApplier;
use crate::utils::paths::expand_tokens;
use genubar_core::MenuError;
use std::collections::BTreeMap;
use std::process::Command;

pub(crate) struct FlatpakApplier<'a> {
    pub app_id: &'a str,
    pub talk_names: &'a [String],
    pub inject_env: &'a BTreeMap<String, String>,
}

impl StatefulStepApplier for FlatpakApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("flatpak:{}", self.app_id),
            format!("talk={:?};env={:?}", self.talk_names, self.inject_env),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(self.app_id, self.talk_names, self.inject_env)
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        _orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.app_id)
    }
}

pub fn apply(
    app_id: &str,
    talk_names: &[String],
    inject_env: &BTreeMap<String, String>,
) -> Result<Option<String>, MenuError> {
    let mut cmd = Command::new("flatpak");
    cmd.args(["override", "--user", app_id]);

    for bus_name in talk_names {
        cmd.arg(format!("--talk-name={bus_name}"));
    }
    if inject_env.contains_key("LD_PRELOAD") {
        let hook_dir = expand_tokens("{HOME}/.local/lib/genubar:ro");
        cmd.arg(format!("--filesystem={hook_dir}"));
    }
    for (k, v) in inject_env {
        let expanded = expand_tokens(v);
        cmd.arg(format!("--env={k}={expanded}"));
    }

    let output = cmd.output().map_err(|e| {
        MenuError::IoPermission(format!("Failed to execute 'flatpak override': {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MenuError::IoPermission(format!(
            "flatpak override failed for '{app_id}': {}",
            stderr.trim()
        )));
    }

    Ok(None)
}

pub fn revert(app_id: &str) -> Result<(), MenuError> {
    let output = Command::new("flatpak")
        .args(["override", "--user", "--reset", app_id])
        .output()
        .map_err(|e| {
            MenuError::IoPermission(format!("Failed to execute 'flatpak override --reset': {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MenuError::IoPermission(format!(
            "flatpak override --reset failed for '{app_id}': {}",
            stderr.trim()
        )));
    }
    Ok(())
}
