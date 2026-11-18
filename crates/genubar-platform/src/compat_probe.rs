use crate::paths::{bin_dir, extensions_dir};
use genubar_core::{EXTENSION_UUID, ExtensionCompatReport, check_extension_metadata_json};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Reads `~/.local/share/gnome-shell/extensions/genubar@github.com/metadata.json` and validates compatibility.
pub fn inspect_installed_extension() -> ExtensionCompatReport {
    let meta_path = extensions_dir().join(EXTENSION_UUID).join("metadata.json");
    let raw = fs::read_to_string(meta_path).ok();
    check_extension_metadata_json(raw.as_deref())
}

/// Runs `<bin_path> --version` and parses the trailing semantic version string.
pub fn query_binary_version(bin_path: &Path) -> Option<String> {
    if !bin_path.is_file() {
        return None;
    }
    let out = Command::new(bin_path).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed = stdout
        .split_whitespace()
        .last()
        .map(|s| s.trim_start_matches('v').to_string())
        .filter(|s| !s.is_empty());
    Some(parsed.unwrap_or_else(|| String::from("unknown")))
}

/// Queries `~/.local/bin/genubar-daemon` (or fallback `genubar-cli`) for the installed version.
pub fn query_installed_daemon_version() -> Option<String> {
    let bin = bin_dir();
    for name in ["genubar-daemon", "genubar-cli"] {
        let cand = bin.join(name);
        if let Some(ver) = query_binary_version(&cand) {
            return Some(ver);
        }
    }
    None
}
