pub use genubar_config::ScriptMetadata;
use genubar_core::{ActiveWindowInfo, MenuNode, MenuSourceTier};
use genubar_platform::fs_utils::read_text_file as read_string;
use genubar_platform::{
    ProcessTimeoutOutcome, parse_metadata_block, resolve_user_path as resolve_config_path,
    run_command_with_stdin_timeout,
};
use std::path::{Path, PathBuf};

const SCRIPT_HEADER_START: &str = "# ==GenubarScript==";
const SCRIPT_HEADER_END: &str = "# ==/GenubarScript==";
pub const DEFAULT_SCRIPT_TIMEOUT_MS: u64 = 500;

pub struct PythonRunner;

impl PythonRunner {
    pub fn resolve_script_path(config_dir: &Path, script_path: &Path) -> PathBuf {
        resolve_config_path(config_dir, script_path)
    }

    pub fn parse_and_validate_header(script_file: &Path) -> Option<ScriptMetadata> {
        let raw = read_string(script_file).ok()?;
        let map = parse_metadata_block(&raw, SCRIPT_HEADER_START, SCRIPT_HEADER_END)?;

        let stem = script_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("script");

        Some(ScriptMetadata {
            id: map.get("id").cloned().unwrap_or_else(|| stem.to_string()),
            name: map
                .get("name")
                .cloned()
                .unwrap_or_else(|| stem.replace('_', " ")),
            kind: map
                .get("kind")
                .cloned()
                .unwrap_or_else(|| String::from("menu_transformer")),
            version: map
                .get("version")
                .cloned()
                .unwrap_or_else(|| String::from("1.0.0")),
            author: map
                .get("author")
                .cloned()
                .unwrap_or_else(|| String::from("local")),
            description: map.get("description").cloned().unwrap_or_default(),
        })
    }

    pub fn run(
        python_bin: &str,
        config_dir: &Path,
        script_path: &Path,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        root: &MenuNode,
    ) -> Option<MenuNode> {
        Self::run_with_timeout(
            python_bin,
            config_dir,
            script_path,
            info,
            tier,
            root,
            DEFAULT_SCRIPT_TIMEOUT_MS,
        )
    }

    /// Executes a `# ==GenubarScript==` transformer script with an enforced execution timeout.
    pub fn run_with_timeout(
        python_bin: &str,
        config_dir: &Path,
        script_path: &Path,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        root: &MenuNode,
        timeout_ms: u64,
    ) -> Option<MenuNode> {
        let resolved_path = Self::resolve_script_path(config_dir, script_path);
        if !resolved_path.is_file() {
            tracing::warn!(path = %resolved_path.display(), "Python script file not found");
            return None;
        }

        if Self::parse_and_validate_header(&resolved_path).is_none() {
            tracing::warn!(
                path = %resolved_path.display(),
                "Ignoring script missing mandatory '{SCRIPT_HEADER_START}' header"
            );
            return None;
        }

        let payload = serde_json::json!({
            "window": info,
            "tier": tier,
            "root": root,
        });
        let input_bytes = serde_json::to_vec(&payload).ok()?;

        let outcome = run_command_with_stdin_timeout(
            python_bin,
            [resolved_path.as_os_str()],
            &input_bytes,
            timeout_ms,
        )
        .ok()?;

        let (success, stdout_buf, stderr_buf) = match outcome {
            ProcessTimeoutOutcome::Completed {
                success,
                stdout,
                stderr,
            } => (success, stdout, stderr),
            ProcessTimeoutOutcome::TimedOut => {
                tracing::warn!(
                    path = %resolved_path.display(),
                    timeout_ms,
                    "Python script exceeded timeout and was terminated"
                );
                return None;
            }
        };

        if !success {
            let err_msg = String::from_utf8_lossy(&stderr_buf);
            tracing::warn!(
                path = %resolved_path.display(),
                error = err_msg.trim(),
                "Python script exited with failure"
            );
            return None;
        }

        match serde_json::from_slice::<MenuNode>(&stdout_buf) {
            Ok(node) => Some(node),
            Err(e) => {
                tracing::warn!(
                    path = %resolved_path.display(),
                    error = %e,
                    "Invalid MenuNode JSON output from Python script"
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::MenuItemType;
    use std::fs;

    fn sample_root() -> MenuNode {
        MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn python_runner_executes_valid_script_and_enforces_timeout() {
        let tmp_dir = std::env::temp_dir().join("genubar_test_python_runner");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let ok_script = tmp_dir.join("fast.py");
        fs::write(
            &ok_script,
            "# ==GenubarScript==\n# @id fast\n# ==/GenubarScript==\nimport json, sys\ndata = json.load(sys.stdin)\nroot = data['root']\nroot['label'] = 'Transformed'\nprint(json.dumps(root))\n",
        )
        .unwrap();

        let slow_script = tmp_dir.join("slow.py");
        fs::write(
            &slow_script,
            "# ==GenubarScript==\n# @id slow\n# ==/GenubarScript==\nimport time, sys, json\ntime.sleep(3)\nprint(json.dumps(json.load(sys.stdin)['root']))\n",
        )
        .unwrap();

        let info = ActiveWindowInfo::default();
        let root = sample_root();

        if let Some(res) = PythonRunner::run_with_timeout(
            "python3",
            &tmp_dir,
            &ok_script,
            &info,
            MenuSourceTier::AppProfile,
            &root,
            2000,
        ) {
            assert_eq!(res.label, "Transformed");

            let started = std::time::Instant::now();
            let timed_out = PythonRunner::run_with_timeout(
                "python3",
                &tmp_dir,
                &slow_script,
                &info,
                MenuSourceTier::AppProfile,
                &root,
                80,
            );
            assert!(timed_out.is_none());
            assert!(started.elapsed() < std::time::Duration::from_millis(1200));
        }

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
