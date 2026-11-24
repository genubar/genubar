use crate::artifacts::{
    check_bundled_extension_compatibility, copy_executable, resolve_or_fetch_artifact,
    search_local_artifact,
};
use crate::error::InstallerError;
use genubar_core::{CURRENT_APP_VERSION, EXPECTED_EXTENSION_VERSION, EXTENSION_UUID};
use genubar_patcher::{PatchLedger, PatchManager};
use genubar_platform::{
    APP_ICON_NAME, EMBEDDED_APP_ICON_SVG, INSTALLER_ICON_NAME, bin_dir, cache_dir, config_dir,
    data_dir, extensions_dir, fs_utils, inspect_installed_extension, install_embedded_extension,
    lib_dir, query_installed_daemon_version, sync_extension_gsettings, systemd_user_dir,
    user_applications_dir, user_icons_scalable_apps_dir,
};
use genubar_protocol::{GTK4_HOOK_ACTIVE_SO_NAME, GTK4_HOOK_CANONICAL_SO_NAME};
use std::fs;
use std::path::Path;
use std::process::Command;

pub const SETTINGS_DESKTOP_FILENAME: &str = "org.genubar.Settings.desktop";

#[derive(Debug, Clone)]
pub struct InstallerStatusSnapshot {
    pub daemon_version: Option<String>,
    pub ext_installed: bool,
    pub ext_compat_ok: bool,
    pub summary_text: String,
}

fn run_best_effort_command(program: &str, args: &[&str], warnings: &mut Vec<String>) {
    match Command::new(program).args(args).output() {
        Ok(out) if !out.status.success() => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !err.is_empty() {
                warnings.push(format!("{program} {}: {err}", args.join(" ")));
            }
        }
        Err(e) => warnings.push(format!("{program} unavailable: {e}")),
        _ => {}
    }
}

fn remove_file_logged(path: &Path, warnings: &mut Vec<String>) {
    if let Err(err) = fs_utils::remove_file_if_exists(path) {
        warnings.push(format!("Could not remove {}: {err}", path.display()));
    }
}

fn remove_dir_logged(path: &Path, warnings: &mut Vec<String>) {
    if let Err(err) = fs_utils::remove_dir_if_exists(path) {
        warnings.push(format!("Could not remove {}: {err}", path.display()));
    }
}

fn query_systemctl_user_state(subcommand: &str) -> String {
    match Command::new("systemctl")
        .args(["--user", subcommand, "genubar.service"])
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else if out.status.success() {
                String::from("ok")
            } else {
                String::from("inactive")
            }
        }
        Err(_) => String::from("unavailable"),
    }
}

fn has_existing_systemd_unit_or_links(systemd_dir: &Path) -> bool {
    systemd_dir.join("genubar.service").exists()
        || systemd_dir
            .join("default.target.wants/genubar.service")
            .symlink_metadata()
            .is_ok()
        || systemd_dir
            .join("graphical-session.target.wants/genubar.service")
            .symlink_metadata()
            .is_ok()
}

pub fn format_systemd_unit(bin_dir: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=Genubar Universal Menubar Daemon\n\
         PartOf=graphical-session.target\n\
         After=graphical-session.target dbus.socket\n\n\
         [Service]\n\
         Environment=\"PATH={}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\"\n\
         ExecStart={}/genubar-daemon daemon\n\
         Restart=on-failure\n\
         RestartSec=2\n\n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        bin_dir.display(),
        bin_dir.display()
    )
}

pub fn format_settings_desktop_entry(bin_dir: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Genubar Settings\n\
         GenericName=Global Menubar Settings\n\
         Comment=Configure Genubar global menubar, styles, app profiles, and patches\n\
         Exec={}/genubar-settings\n\
         Icon={APP_ICON_NAME}\n\
         Terminal=false\n\
         Categories=GTK;GNOME;Settings;DesktopSettings;Utility;\n\
         Keywords=genubar;menubar;global menu;panel;gnome;\n\
         StartupNotify=true\n",
        bin_dir.display()
    )
}

pub fn install_extension_only() -> Result<String, InstallerError> {
    install_embedded_extension(false).map_err(|e| InstallerError::Extension(e.to_string()))
}

/// Stops systemd service, installs binaries & embedded GNOME extension, enables autostart, and starts daemon.
pub fn perform_install_or_update() -> Result<String, InstallerError> {
    let ext_ver = check_bundled_extension_compatibility()?;

    let bin_dir = bin_dir();
    let lib_dir = lib_dir();
    let data_dir = data_dir();
    let systemd_dir = systemd_user_dir();

    for d in [&bin_dir, &lib_dir, &data_dir, &systemd_dir] {
        fs_utils::ensure_dir(d).map_err(|e| InstallerError::Io {
            context: format!("create directory {}", d.display()),
            source: std::io::Error::other(e.to_string()),
        })?;
    }

    let mut warnings = Vec::new();
    let service_path = systemd_dir.join("genubar.service");
    if has_existing_systemd_unit_or_links(&systemd_dir) {
        // Disable before rewriting the unit so stale target wants symlinks (e.g. default.target) are cleared.
        run_best_effort_command(
            "systemctl",
            &["--user", "disable", "--now", "genubar.service"],
            &mut warnings,
        );
    }

    let daemon_src = resolve_or_fetch_artifact(&["genubar-daemon", "genubar-cli"])?;
    copy_executable(&daemon_src, &bin_dir.join("genubar-daemon"))?;

    if let Some(cli_src) = search_local_artifact(&["genubar-cli"]) {
        copy_executable(&cli_src, &bin_dir.join("genubar-cli"))?;
    } else if daemon_src.file_name().and_then(|n| n.to_str()) == Some("genubar-cli") {
        copy_executable(&daemon_src, &bin_dir.join("genubar-cli"))?;
    } else {
        warnings.push(String::from(
            "genubar-cli binary not found; skipped CLI install",
        ));
    }

    if let Some(ui_src) = search_local_artifact(&["genubar-settings"]) {
        copy_executable(&ui_src, &bin_dir.join("genubar-settings"))?;
    } else if let Ok(ui_src) = resolve_or_fetch_artifact(&["genubar-settings"]) {
        copy_executable(&ui_src, &bin_dir.join("genubar-settings"))?;
    } else {
        warnings.push(String::from(
            "genubar-settings binary not found; skipped GUI install",
        ));
    }

    if bin_dir.join("genubar-settings").is_file() {
        let icons_dir = user_icons_scalable_apps_dir();
        if let Err(err) = fs_utils::ensure_dir(&icons_dir) {
            warnings.push(format!("Could not create {}: {err}", icons_dir.display()));
        } else {
            for icon_name in [APP_ICON_NAME, INSTALLER_ICON_NAME] {
                let icon_path = icons_dir.join(format!("{icon_name}.svg"));
                if let Err(err) = fs::write(&icon_path, EMBEDDED_APP_ICON_SVG) {
                    warnings.push(format!("Could not write {}: {err}", icon_path.display()));
                }
            }
        }

        let apps_dir = user_applications_dir();
        if let Err(err) = fs_utils::ensure_dir(&apps_dir) {
            warnings.push(format!("Could not create {}: {err}", apps_dir.display()));
        } else {
            let desktop_path = apps_dir.join(SETTINGS_DESKTOP_FILENAME);
            if let Err(err) = fs::write(&desktop_path, format_settings_desktop_entry(&bin_dir)) {
                warnings.push(format!("Could not write {}: {err}", desktop_path.display()));
            }
        }
    }

    if let Some(hook_src) = search_local_artifact(&[GTK4_HOOK_CANONICAL_SO_NAME]) {
        for target_name in [GTK4_HOOK_CANONICAL_SO_NAME, GTK4_HOOK_ACTIVE_SO_NAME] {
            let dst = lib_dir.join(target_name);
            fs::copy(&hook_src, &dst).map_err(|source| InstallerError::Io {
                context: format!("copy GTK4 hook {} -> {}", hook_src.display(), dst.display()),
                source,
            })?;
        }
    } else {
        warnings.push(String::from(
            "libgenubar_gtk4_hook.so not found; skipped hook install",
        ));
    }

    install_extension_only()?;

    let service_unit = format_systemd_unit(&bin_dir);
    fs::write(&service_path, service_unit).map_err(|source| InstallerError::Io {
        context: format!("write systemd unit {}", service_path.display()),
        source,
    })?;

    run_best_effort_command("systemctl", &["--user", "daemon-reload"], &mut warnings);
    let _ = Command::new("systemctl")
        .args(["--user", "reset-failed", "genubar.service"])
        .output();
    run_best_effort_command(
        "systemctl",
        &["--user", "enable", "genubar.service"],
        &mut warnings,
    );
    run_best_effort_command(
        "systemctl",
        &["--user", "restart", "genubar.service"],
        &mut warnings,
    );

    let enabled_state = query_systemctl_user_state("is-enabled");
    let active_state = query_systemctl_user_state("is-active");
    if enabled_state != "enabled" {
        warnings.push(format!(
            "genubar.service autostart state is '{enabled_state}'"
        ));
    }
    if active_state != "active" {
        warnings.push(format!("genubar.service runtime state is '{active_state}'"));
    }

    let warn_note = if warnings.is_empty() {
        String::new()
    } else {
        format!("\nNotes: {}", warnings.join("; "))
    };
    let service_summary = if enabled_state == "enabled" && active_state == "active" {
        "genubar.service enabled & running"
    } else {
        "genubar.service configured (check notes below)"
    };
    Ok(format!(
        "Installed Genubar v{CURRENT_APP_VERSION} (from {}) & Extension v{ext_ver}; {service_summary}!{warn_note}",
        daemon_src.display()
    ))
}

/// Stops and removes daemon, extension, systemd service, reverts applied patches, and optionally purges data.
pub fn perform_uninstall(purge_configs_and_cache: bool) -> Result<String, InstallerError> {
    let bin_dir = bin_dir();
    let systemd_dir = systemd_user_dir();
    let mut warnings = Vec::new();
    let reverted_patches = revert_applied_patches_before_uninstall(&mut warnings);
    if has_existing_systemd_unit_or_links(&systemd_dir) {
        run_best_effort_command(
            "systemctl",
            &["--user", "disable", "--now", "genubar.service"],
            &mut warnings,
        );
    }
    if extensions_dir().join(EXTENSION_UUID).exists() {
        let _ = Command::new("gnome-extensions")
            .args(["disable", EXTENSION_UUID])
            .status();
    }
    sync_extension_gsettings(None, Some(EXTENSION_UUID));

    remove_file_logged(&systemd_dir.join("genubar.service"), &mut warnings);
    remove_file_logged(&bin_dir.join("genubar-daemon"), &mut warnings);
    remove_file_logged(&bin_dir.join("genubar-cli"), &mut warnings);
    remove_file_logged(&bin_dir.join("genubar-settings"), &mut warnings);
    remove_file_logged(
        &user_applications_dir().join(SETTINGS_DESKTOP_FILENAME),
        &mut warnings,
    );
    for icon_name in [APP_ICON_NAME, INSTALLER_ICON_NAME] {
        remove_file_logged(
            &user_icons_scalable_apps_dir().join(format!("{icon_name}.svg")),
            &mut warnings,
        );
    }
    remove_dir_logged(&lib_dir(), &mut warnings);
    remove_dir_logged(&extensions_dir().join(EXTENSION_UUID), &mut warnings);

    if purge_configs_and_cache {
        remove_dir_logged(&config_dir(), &mut warnings);
        remove_dir_logged(&cache_dir(), &mut warnings);
        remove_dir_logged(&data_dir(), &mut warnings);
    }

    let revert_note = if reverted_patches > 0 {
        format!(", reverted {reverted_patches} applied patch(es)")
    } else {
        String::new()
    };
    let summary = if purge_configs_and_cache {
        format!(
            "Uninstalled Genubar, GNOME extension{revert_note}, and purged all configs & caches."
        )
    } else {
        format!(
            "Uninstalled Genubar binaries & extension{revert_note} (user configs in ~/.config/genubar preserved)."
        )
    };
    if warnings.is_empty() {
        Ok(summary)
    } else {
        Ok(format!("{summary}\nWarnings: {}", warnings.join("; ")))
    }
}

fn revert_applied_patches_before_uninstall(warnings: &mut Vec<String>) -> usize {
    if !PatchLedger::ledger_path().is_file() {
        return 0;
    }
    let peek = PatchLedger::load_or_default();
    if peek.applied_patches.is_empty() {
        return 0;
    }
    match PatchManager::load_or_init() {
        Ok(mut mgr) => revert_all_applied_patches_in_manager(&mut mgr, warnings),
        Err(err) => {
            warnings.push(format!(
                "Could not load patch manager to revert patches: {err}"
            ));
            0
        }
    }
}

fn revert_all_applied_patches_in_manager(
    mgr: &mut PatchManager,
    warnings: &mut Vec<String>,
) -> usize {
    let mut patch_ids: Vec<String> = mgr.ledger.applied_patches.iter().cloned().collect();
    if patch_ids.is_empty() {
        return 0;
    }
    patch_ids.sort();
    let mut reverted = 0usize;
    for patch_id in patch_ids {
        match mgr.revert_patch_ext(&patch_id, true) {
            Ok(step_warnings) => {
                reverted += 1;
                for w in step_warnings {
                    warnings.push(format!("Patch '{patch_id}': {w}"));
                }
            }
            Err(err) => {
                warnings.push(format!("Failed reverting patch '{patch_id}': {err}"));
            }
        }
    }
    reverted
}

pub fn inspect_status_snapshot() -> InstallerStatusSnapshot {
    let daemon_version = query_installed_daemon_version();
    let ext_info = inspect_installed_extension();
    let ext_compat = check_bundled_extension_compatibility();
    let ext_compat_ok = ext_compat.is_ok();
    let ext_compat_str = match &ext_compat {
        Ok(v) => {
            format!("Bundled Extension v{v} (Expected: v{EXPECTED_EXTENSION_VERSION}, compatible)")
        }
        Err(e) => format!("Extension incompatibility: {e}"),
    };
    let installed_ext_str = if ext_info.installed {
        format!("Installed Extension: v{}", ext_info.version)
    } else {
        String::from("Installed Extension: Not installed")
    };
    let enabled_state = query_systemctl_user_state("is-enabled");
    let active_state = query_systemctl_user_state("is-active");
    let service_str =
        format!("Systemd Service: autostart {enabled_state} | background {active_state}");
    let summary_text = match &daemon_version {
        Some(v) => format!(
            "Installed Daemon: v{v} | Package: v{CURRENT_APP_VERSION}\n{service_str}\n{installed_ext_str}\n{ext_compat_str}"
        ),
        None => format!(
            "Daemon: Not installed | Package: v{CURRENT_APP_VERSION}\n{service_str}\n{installed_ext_str}\n{ext_compat_str}"
        ),
    };

    InstallerStatusSnapshot {
        daemon_version,
        ext_installed: ext_info.installed,
        ext_compat_ok,
        summary_text,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_settings_desktop_entry, format_systemd_unit, inspect_status_snapshot,
        revert_all_applied_patches_in_manager,
    };
    use genubar_core::PatchErrorPolicy;
    use genubar_patcher::{
        ConflictStrategy, PackageMetadata, PatchLedger, PatchManager, PatchManifest, PatchStep,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn status_snapshot_formats_without_panicking() {
        let snap = inspect_status_snapshot();
        assert!(snap.ext_compat_ok);
        assert!(snap.summary_text.contains("Package: v"));
    }

    #[test]
    fn generated_and_reference_systemd_units_match_wayland_autostart_contract() {
        let generated = format_systemd_unit(Path::new("/home/demo/.local/bin"));
        let reference = include_str!("../../../systemd/genubar.service");

        for unit in [&generated, &reference.to_string()] {
            assert!(unit.contains("PartOf=graphical-session.target"));
            assert!(unit.contains("After=graphical-session.target dbus.socket"));
            assert!(unit.contains("Restart=on-failure"));
            assert!(unit.contains("WantedBy=graphical-session.target"));
            assert!(!unit.contains("WantedBy=default.target"));
        }
        assert!(generated.contains("Environment=\"PATH=/home/demo/.local/bin:"));
        assert!(generated.contains("ExecStart=/home/demo/.local/bin/genubar-daemon daemon"));
        assert!(reference.contains("Environment=\"PATH=%h/.local/bin:"));
        assert!(reference.contains("ExecStart=%h/.local/bin/genubar-daemon daemon"));

        let desktop = format_settings_desktop_entry(Path::new("/home/demo/.local/bin"));
        assert!(desktop.contains("Name=Genubar Settings"));
        assert!(desktop.contains("Exec=/home/demo/.local/bin/genubar-settings"));
        assert!(desktop.contains("Icon=org.genubar.Settings"));
        assert!(desktop.contains("Categories=GTK;GNOME;Settings;DesktopSettings;Utility;"));
        assert!(!genubar_platform::EMBEDDED_APP_ICON_SVG.is_empty());
    }

    #[test]
    fn revert_all_applied_patches_in_manager_restores_modified_files_in_temp_dir() {
        let tmp = std::env::temp_dir().join("genubar_test_uninstall_revert");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("patches")).unwrap();

        let vmoptions_path = tmp.join("rustrover64.vmoptions");
        fs::write(&vmoptions_path, "-Xms256m\n").unwrap();

        let manifest = PatchManifest {
            meta: PackageMetadata {
                id: String::from("demo_ide_patch"),
                name: String::from("Demo IDE Patch"),
                ..Default::default()
            },
            detection: Default::default(),
            steps: vec![PatchStep::TextLineAppend {
                file_pattern: vmoptions_path.to_string_lossy().to_string(),
                line: String::from("-DjbScreenMenuBar.enabled=true"),
            }],
        };

        let mut mgr = PatchManager {
            packages_dir: tmp.join("patches"),
            patches: vec![manifest],
            ledger: PatchLedger::load_or_default_at(tmp.join("patch_ledger.json")),
        };

        mgr.apply_patch_ext(
            "demo_ide_patch",
            ConflictStrategy::Abort,
            PatchErrorPolicy::Strict,
            false,
        )
        .unwrap();
        assert!(
            fs::read_to_string(&vmoptions_path)
                .unwrap()
                .contains("-DjbScreenMenuBar.enabled=true")
        );
        assert!(mgr.ledger.applied_patches.contains("demo_ide_patch"));

        let mut warnings = Vec::new();
        let count = revert_all_applied_patches_in_manager(&mut mgr, &mut warnings);
        assert_eq!(count, 1);
        assert!(warnings.is_empty());
        assert!(mgr.ledger.applied_patches.is_empty());
        assert_eq!(fs::read_to_string(&vmoptions_path).unwrap(), "-Xms256m\n");

        let _ = fs::remove_dir_all(&tmp);
    }
}
