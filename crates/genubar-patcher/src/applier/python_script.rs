use super::StatefulStepApplier;
use genubar_core::MenuError;
use genubar_platform::fs_utils::read_text_file;
use genubar_platform::{
    ProcessTimeoutOutcome, config_dir, home_dir, parse_metadata_block, resolve_user_path,
    run_command_with_stdin_timeout,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct PythonScriptApplier<'a> {
    pub script: &'a Path,
    pub args: &'a BTreeMap<String, String>,
}

impl StatefulStepApplier for PythonScriptApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("python_script:{}:{:?}", self.script.display(), self.args),
            String::from("applied"),
        )
    }

    fn apply_change(&self, patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(patch_id, self.script, self.args)
    }

    fn revert_change(
        &self,
        patch_id: &str,
        orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(patch_id, self.script, self.args, orig_opt)
    }
}

const SCRIPT_HEADER_START: &str = "# ==GenubarScript==";
const SCRIPT_HEADER_END: &str = "# ==/GenubarScript==";
const DEFAULT_PATCH_SCRIPT_TIMEOUT_MS: u64 = 15_000;

const PYTHON_BRIDGE_SNIPPET: &str = r#"
import importlib.util, json, sys
payload = json.load(sys.stdin)
spec = importlib.util.spec_from_file_location("genubar_patch_mod", payload["script_path"])
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
mode = payload["mode"]
ctx = payload["ctx"]
if mode == "apply":
    if not hasattr(mod, "apply"):
        raise RuntimeError("Patch script is missing required 'def apply(ctx):' function")
    res = mod.apply(ctx)
    print(json.dumps({"saved_state": res}))
elif mode == "revert":
    if not hasattr(mod, "revert"):
        raise RuntimeError("Patch script is missing required 'def revert(ctx, saved_state):' function")
    mod.revert(ctx, payload.get("saved_state"))
    print(json.dumps({"saved_state": None}))
"#;

pub fn apply(
    patch_id: &str,
    script: &Path,
    args: &BTreeMap<String, String>,
) -> Result<Option<String>, MenuError> {
    run_script(patch_id, script, args, "apply", None)
}

pub fn revert(
    patch_id: &str,
    script: &Path,
    args: &BTreeMap<String, String>,
    saved_state: Option<String>,
) -> Result<(), MenuError> {
    let _ = run_script(patch_id, script, args, "revert", saved_state)?;
    Ok(())
}

fn run_script(
    patch_id: &str,
    script: &Path,
    args: &BTreeMap<String, String>,
    mode: &str,
    saved_state: Option<String>,
) -> Result<Option<String>, MenuError> {
    let home = home_dir();
    let cfg_dir = config_dir();
    let resolved = resolve_user_path(&cfg_dir, script);

    if !resolved.is_file() {
        return Err(MenuError::IoPermission(format!(
            "Patch Python script not found: {}",
            resolved.display()
        )));
    }

    let raw_script = read_text_file(&resolved)?;
    let header_map = parse_metadata_block(&raw_script, SCRIPT_HEADER_START, SCRIPT_HEADER_END)
        .ok_or_else(|| {
            MenuError::CheckFailed(format!(
                "Python script {} is missing mandatory '{SCRIPT_HEADER_START}' header",
                resolved.display()
            ))
        })?;
    let stem = resolved
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("script");
    let script_id = header_map
        .get("id")
        .cloned()
        .unwrap_or_else(|| stem.to_string());

    let input_json = serde_json::json!({
        "script_path": resolved.to_string_lossy(),
        "mode": mode,
        "saved_state": saved_state,
        "ctx": {
            "patch_id": patch_id,
            "script_id": script_id,
            "home": home.to_string_lossy(),
            "args": args,
        }
    });

    let stdin_payload = input_json.to_string();
    let outcome = run_command_with_stdin_timeout(
        "python3",
        ["-c", PYTHON_BRIDGE_SNIPPET],
        stdin_payload.as_bytes(),
        DEFAULT_PATCH_SCRIPT_TIMEOUT_MS,
    )
    .map_err(|e| MenuError::IoPermission(format!("Failed to execute python3: {e}")))?;

    let (success, stdout, stderr) = match outcome {
        ProcessTimeoutOutcome::Completed {
            success,
            stdout,
            stderr,
        } => (success, stdout, stderr),
        ProcessTimeoutOutcome::TimedOut => {
            return Err(MenuError::IoPermission(format!(
                "Python script '{}' ({mode}) timed out after {DEFAULT_PATCH_SCRIPT_TIMEOUT_MS}ms",
                resolved.display()
            )));
        }
    };

    if !success {
        let stderr_str = String::from_utf8_lossy(&stderr);
        return Err(MenuError::IoPermission(format!(
            "Python script '{}' ({mode}) failed: {}",
            resolved.display(),
            stderr_str.trim()
        )));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&stdout).map_err(|e| {
        MenuError::ParseError(format!(
            "Invalid JSON output from patch script {}: {e}",
            resolved.display()
        ))
    })?;

    let state_str = parsed.get("saved_state").and_then(|v| {
        if v.is_null() {
            None
        } else if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else {
            Some(v.to_string())
        }
    });

    Ok(state_str)
}