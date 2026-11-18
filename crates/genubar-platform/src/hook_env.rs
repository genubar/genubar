use crate::fs_utils::{ensure_dir, replace_with_atomic_copy_or_symlink};
use crate::paths::{lib_dir, resolve_user_path};
use genubar_core::MenuError;
use genubar_protocol::{GTK4_HOOK_ACTIVE_SO_NAME, GTK4_HOOK_SO_NAMES, HIDDEN_BUTTONS_ENV};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Finds the most recently modified `libgenubar_gtk4_hook.so` candidate across workspace
/// target directories and `~/.local/lib/genubar`.
pub fn newest_gtk4_hook_so() -> Option<PathBuf> {
    let lib = lib_dir();
    let mut candidates = Vec::new();
    #[cfg(debug_assertions)]
    {
        candidates.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/debug/libgenubar_gtk4_hook.so"
        )));
        candidates.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/release/libgenubar_gtk4_hook.so"
        )));
    }
    candidates.push(lib.join("libgenubar_gtk4_hook.so"));
    candidates.push(lib.join("libmenu_gtk4_hook.so"));

    candidates
        .into_iter()
        .filter(|p| p.is_file())
        .max_by_key(|p| {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
}

/// Atomically copies the newest `libgenubar_gtk4_hook.so` candidate to
/// `~/.local/lib/genubar/active_gtk4_hook.so` (for Flatpak compatibility) and returns the active path.
pub fn locate_or_sync_gtk4_hook_so() -> String {
    let active_so = lib_dir().join(GTK4_HOOK_ACTIVE_SO_NAME);
    if let Some(newest_so) = newest_gtk4_hook_so() {
        if let Some(parent) = active_so.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if newest_so != active_so {
            let tmp_path = active_so.with_extension("so.tmp");
            if fs::copy(&newest_so, &tmp_path).is_ok() {
                let _ = fs::remove_file(&active_so);
                if fs::rename(&tmp_path, &active_so).is_ok() {
                    return active_so.to_string_lossy().to_string();
                }
            }
        }
        return newest_so.to_string_lossy().to_string();
    }
    active_so.to_string_lossy().to_string()
}

/// Finds an installed GTK4 hook shared library inside `~/.local/lib/genubar`.
pub fn resolve_installed_gtk4_hook_so() -> Option<String> {
    let base = lib_dir();
    for name in GTK4_HOOK_SO_NAMES {
        let cand = base.join(name);
        if cand.is_file() {
            return Some(cand.to_string_lossy().to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
pub struct Gtk4EnvSyncOptions<'a> {
    pub hide_original_buttons: bool,
    pub custom_hook_so_path: Option<&'a Path>,
}

pub struct Gtk4EnvSynchronizer;

impl Gtk4EnvSynchronizer {
    pub fn sync(base_dir: &Path, opts: &Gtk4EnvSyncOptions<'_>) {
        if let Err(err) = Self::try_sync(base_dir, opts) {
            tracing::warn!(error = %err, "Failed syncing GTK4 hook environment");
        }
    }

    pub fn try_sync(base_dir: &Path, opts: &Gtk4EnvSyncOptions<'_>) -> Result<(), MenuError> {
        let lib = lib_dir();
        ensure_dir(&lib)?;

        let symlink_path = lib.join(GTK4_HOOK_ACTIVE_SO_NAME);
        let target_so = Self::resolve_target_so(base_dir, opts);

        if target_so.exists() {
            replace_with_atomic_copy_or_symlink(&target_so, &symlink_path)?;
        }

        let hide_val = if opts.hide_original_buttons { "1" } else { "0" };

        match Command::new("dbus-update-activation-environment")
            .args(["--systemd", &format!("{HIDDEN_BUTTONS_ENV}={hide_val}")])
            .status()
        {
            Ok(st) if !st.success() => {
                tracing::warn!(status = ?st, "dbus-update-activation-environment exited non-zero");
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed invoking dbus-update-activation-environment");
            }
            Ok(_) => {}
        }

        Ok(())
    }

    fn resolve_target_so(base_dir: &Path, opts: &Gtk4EnvSyncOptions<'_>) -> PathBuf {
        if let Some(custom_path) = opts.custom_hook_so_path {
            let expanded = resolve_user_path(base_dir, custom_path);
            if expanded.exists() {
                return expanded;
            }
        }

        newest_gtk4_hook_so().unwrap_or_else(|| lib_dir().join(GTK4_HOOK_ACTIVE_SO_NAME))
    }
}
