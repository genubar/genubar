use crate::compat_probe::inspect_installed_extension;
use crate::fs_utils::{ensure_dir, write_bytes_file, write_text_file};
use crate::paths::extensions_dir;
use genubar_core::{EXTENSION_UUID, MenuError, check_extension_metadata_json};
use std::fs;
use std::process::Command;

/// Bundled GNOME Shell extension sources shared across installer and settings UI.
pub const EMBEDDED_EXT_FILES: &[(&str, &str)] = &[
    (
        "metadata.json",
        include_str!("../../../genubar-gnome-extension/metadata.json"),
    ),
    (
        "extension.js",
        include_str!("../../../genubar-gnome-extension/extension.js"),
    ),
    (
        "stylesheet.css",
        include_str!("../../../genubar-gnome-extension/stylesheet.css"),
    ),
    (
        "lib/core/l0Cache.js",
        include_str!("../../../genubar-gnome-extension/lib/core/l0Cache.js"),
    ),
    (
        "lib/core/menuModel.js",
        include_str!("../../../genubar-gnome-extension/lib/core/menuModel.js"),
    ),
    (
        "lib/core/menuStore.js",
        include_str!("../../../genubar-gnome-extension/lib/core/menuStore.js"),
    ),
    (
        "lib/core/slotLayout.js",
        include_str!("../../../genubar-gnome-extension/lib/core/slotLayout.js"),
    ),
    (
        "lib/core/textUtils.js",
        include_str!("../../../genubar-gnome-extension/lib/core/textUtils.js"),
    ),
    (
        "lib/ipc/dbusClient.js",
        include_str!("../../../genubar-gnome-extension/lib/ipc/dbusClient.js"),
    ),
    (
        "lib/ipc/versionCheck.js",
        include_str!("../../../genubar-gnome-extension/lib/ipc/versionCheck.js"),
    ),
    (
        "lib/platform/actionRegistry.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/actionRegistry.js"),
    ),
    (
        "lib/platform/focusTracker.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/focusTracker.js"),
    ),
    (
        "lib/platform/panelAdapter.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/panelAdapter.js"),
    ),
    (
        "lib/platform/shortcuts.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/shortcuts.js"),
    ),
    (
        "lib/platform/themeManager.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/themeManager.js"),
    ),
    (
        "lib/platform/windowActions.js",
        include_str!("../../../genubar-gnome-extension/lib/platform/windowActions.js"),
    ),
    (
        "lib/ui/cascadeController.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/cascadeController.js"),
    ),
    (
        "lib/ui/iconCache.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/iconCache.js"),
    ),
    (
        "lib/ui/menuBar.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/menuBar.js"),
    ),
    (
        "lib/ui/menuButton.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/menuButton.js"),
    ),
    (
        "lib/ui/menuColumn.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/menuColumn.js"),
    ),
    (
        "lib/ui/menuRow.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/menuRow.js"),
    ),
    (
        "lib/ui/searchWidget.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/searchWidget.js"),
    ),
    (
        "lib/ui/windowListExpander.js",
        include_str!("../../../genubar-gnome-extension/lib/ui/windowListExpander.js"),
    ),
];

const LEGACY_EXTENSION_FILES: &[&str] = &[
    "renderer.js",
    "theme.js",
    "actions.js",
    "search.js",
    "lib/actions.js",
    "lib/dbusClient.js",
    "lib/icons.js",
    "lib/l0Cache.js",
    "lib/menuRow.js",
    "lib/panelLayout.js",
    "lib/renderer.js",
    "lib/search.js",
    "lib/shortcuts.js",
    "lib/submenuController.js",
    "lib/theme.js",
    "lib/utils.js",
    "lib/versionCheck.js",
    "lib/windowActions.js",
    "lib/windowList.js",
];

fn cleanup_legacy_extension_files(ext_dir: &std::path::Path) {
    for legacy in LEGACY_EXTENSION_FILES {
        let path = ext_dir.join(legacy);
        if path.is_file() {
            let _ = fs::remove_file(path);
        }
    }
}

fn parse_gvariant_str_array(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "@as []" || trimmed == "[]" {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    let mut items = Vec::new();
    for part in inner.split(',') {
        let item = part.trim();
        let unquoted = item
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| item.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
            .unwrap_or(item);
        if !unquoted.is_empty() {
            items.push(unquoted.to_string());
        }
    }
    items
}

/// Computes an updated GSettings string-array literal (`['a', 'b']`) or `None` when already up to date.
pub fn update_gsettings_extension_list(
    raw_list: &str,
    enable_uuid: Option<&str>,
    remove_uuid: Option<&str>,
) -> Option<String> {
    let mut items = parse_gvariant_str_array(raw_list);
    let original = items.clone();

    if let Some(rm) = remove_uuid {
        items.retain(|u| u != rm);
    }
    if let Some(add) = enable_uuid
        && !items.iter().any(|u| u == add)
    {
        items.push(add.to_string());
    }

    if items == original {
        return None;
    }

    if items.is_empty() {
        Some(String::from("@as []"))
    } else {
        let joined = items
            .iter()
            .map(|u| format!("'{}'", u.replace('\'', "\\'")))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("[{joined}]"))
    }
}

/// Ensures `enable_uuid` / `remove_uuid` is reflected in `org.gnome.shell enabled-extensions` via `gsettings`.
/// Needed on GNOME Wayland when installing a fresh extension before Shell rescans `~/.local/share/gnome-shell/extensions`.
pub fn sync_extension_gsettings(enable_uuid: Option<&str>, remove_uuid: Option<&str>) {
    let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.shell", "enabled-extensions"])
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let current = String::from_utf8_lossy(&out.stdout);
    if let Some(updated) = update_gsettings_extension_list(&current, enable_uuid, remove_uuid) {
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.shell", "enabled-extensions", &updated])
            .status();
    }
}

/// Validates the bundled `metadata.json` against the current Daemon compatibility contract.
pub fn check_embedded_extension_compatibility() -> Result<String, MenuError> {
    let report = check_extension_metadata_json(Some(EMBEDDED_EXT_FILES[0].1));
    if report.is_compatible {
        Ok(report.version)
    } else {
        Err(MenuError::Config(report.status_summary))
    }
}

/// Installs the GNOME Shell extension from embedded sources, checking local workspace files first
/// when `prefer_workspace_local` is true.
pub fn install_embedded_extension(prefer_workspace_local: bool) -> Result<String, MenuError> {
    let ext_ver = check_embedded_extension_compatibility()?;
    let ext_dir = extensions_dir().join(EXTENSION_UUID);
    ensure_dir(&ext_dir)?;
    cleanup_legacy_extension_files(&ext_dir);

    #[cfg(debug_assertions)]
    let workspace_ext_dir = Some(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../genubar-gnome-extension"),
    );
    #[cfg(not(debug_assertions))]
    let workspace_ext_dir: Option<std::path::PathBuf> = None;

    for (fname, embedded_body) in EMBEDDED_EXT_FILES {
        if let Some(ref local_dir) = workspace_ext_dir {
            let local_src = local_dir.join(fname);
            if prefer_workspace_local && local_src.is_file() {
                let bytes = fs::read(&local_src).map_err(|e| {
                    MenuError::IoPermission(format!("Failed reading {}: {e}", local_src.display()))
                })?;
                write_bytes_file(ext_dir.join(fname), &bytes)?;
                continue;
            }
        }
        write_text_file(ext_dir.join(fname), embedded_body)?;
    }

    let enabled_via_cli = match Command::new("gnome-extensions")
        .args(["enable", EXTENSION_UUID])
        .status()
    {
        Ok(st) if st.success() => true,
        Ok(st) => {
            tracing::debug!(status = ?st, "gnome-extensions enable exited non-zero; syncing gsettings directly");
            false
        }
        Err(err) => {
            tracing::debug!(error = %err, "gnome-extensions CLI unavailable; syncing gsettings directly");
            false
        }
    };
    if !enabled_via_cli {
        sync_extension_gsettings(Some(EXTENSION_UUID), None);
    }

    let info = inspect_installed_extension();
    if info.installed && info.version != "unknown" && info.version != "0.0.0" {
        Ok(info.version)
    } else {
        Ok(ext_ver)
    }
}

/// Writes an atomic pre-fetched list of extension `(filename, bytes)` files to `extensions_dir()`.
pub fn write_downloaded_extension_files(files: &[(&str, Vec<u8>)]) -> Result<String, MenuError> {
    let ext_dir = extensions_dir().join(EXTENSION_UUID);
    ensure_dir(&ext_dir)?;
    cleanup_legacy_extension_files(&ext_dir);

    for (fname, bytes) in files {
        write_bytes_file(ext_dir.join(fname), bytes)?;
    }

    Ok(inspect_installed_extension().version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::{EXPECTED_EXTENSION_VERSION, EXTENSION_FILES};

    #[test]
    fn embedded_extension_files_match_protocol_list_and_are_compatible() {
        assert_eq!(EMBEDDED_EXT_FILES.len(), EXTENSION_FILES.len());
        for ((name, body), expected_name) in EMBEDDED_EXT_FILES.iter().zip(EXTENSION_FILES.iter()) {
            assert_eq!(name, expected_name);
            assert!(!body.trim().is_empty(), "Embedded file {name} is empty");
        }
        let ver = check_embedded_extension_compatibility().unwrap();
        assert_eq!(ver, EXPECTED_EXTENSION_VERSION);
    }

    #[test]
    fn cleanup_legacy_extension_files_removes_flat_files_and_keeps_layered_modules() {
        let tmp_dir = std::env::temp_dir().join("genubar_test_cleanup_legacy_ext");
        let _ = fs::remove_dir_all(&tmp_dir);
        write_text_file(tmp_dir.join("renderer.js"), "legacy").unwrap();
        write_text_file(tmp_dir.join("lib/utils.js"), "legacy flat").unwrap();
        write_text_file(tmp_dir.join("lib/core/textUtils.js"), "current").unwrap();

        cleanup_legacy_extension_files(&tmp_dir);

        assert!(!tmp_dir.join("renderer.js").exists());
        assert!(!tmp_dir.join("lib/utils.js").exists());
        assert!(tmp_dir.join("lib/core/textUtils.js").is_file());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn update_gsettings_extension_list_handles_empty_add_remove_and_idempotency() {
        assert_eq!(
            update_gsettings_extension_list("@as []", Some("genubar@github.com"), None),
            Some(String::from("['genubar@github.com']"))
        );
        assert_eq!(
            update_gsettings_extension_list(
                "['dash-to-dock@micxgx.gmail.com', 'other@example.com']",
                Some("genubar@github.com"),
                Some("other@example.com")
            ),
            Some(String::from(
                "['dash-to-dock@micxgx.gmail.com', 'genubar@github.com']"
            ))
        );
        assert_eq!(
            update_gsettings_extension_list(
                "['dash-to-dock@micxgx.gmail.com', 'genubar@github.com']",
                Some("genubar@github.com"),
                Some("other@example.com")
            ),
            None
        );
        assert_eq!(
            update_gsettings_extension_list(
                "['genubar@github.com']",
                None,
                Some("genubar@github.com")
            ),
            Some(String::from("@as []"))
        );
    }
}
