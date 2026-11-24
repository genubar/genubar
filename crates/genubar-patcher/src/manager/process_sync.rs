use crate::models::PatchStep;
use crate::utils::paths::expand_tokens;
use genubar_platform::desktop_probe::{
    read_desktop_exec_bin, system_dbus_service_path, user_dbus_service_path,
};
use genubar_platform::fs_utils::{
    ensure_dir, read_text_file, remove_file_if_exists, write_text_file,
};
use genubar_platform::proc_utils::terminate_unhooked_processes;
use genubar_platform::{user_applications_dir, user_dbus_services_dir};
use std::collections::BTreeMap;
use std::process::Command;

// For every `DesktopOverride` step:
// 1. Overrides `/usr/share/dbus-1/services/<app_id>.service` in `~/.local/share/dbus-1/services/`
//    so D-Bus-activated background daemons also get LD_PRELOAD.
// 2. Terminates any stale unhooked background `--gapplication-service` process so the next launch uses the hook.
// 3. Refreshes `~/.local/share/applications` desktop database cache and reloads D-Bus session config.
pub(super) fn sync_dbus_services_and_kill_stale_daemons(steps: &[PatchStep], is_apply: bool) {
    let user_dbus_dir = user_dbus_services_dir();
    if let Err(err) = ensure_dir(&user_dbus_dir) {
        tracing::warn!(
            path = %user_dbus_dir.display(),
            error = %err,
            "Failed to create user D-Bus services directory"
        );
    }

    for step in steps {
        let PatchStep::DesktopOverride {
            desktop_file,
            aliases,
            inject_env,
            ..
        } = step
        else {
            continue;
        };

        for candidate in std::iter::once(desktop_file).chain(aliases.iter()) {
            let app_id = candidate.trim_end_matches(".desktop");
            let sys_service = system_dbus_service_path(app_id);
            let user_service = user_dbus_service_path(app_id);

            if is_apply {
                if sys_service.is_file() {
                    match read_text_file(&sys_service) {
                        Ok(raw) => {
                            if let Some(patched) = inject_env_into_dbus_service(&raw, inject_env)
                                && let Err(err) = write_text_file(&user_service, &patched)
                            {
                                tracing::warn!(
                                    path = %user_service.display(),
                                    error = %err,
                                    "Failed to write patched user D-Bus service override"
                                );
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                path = %sys_service.display(),
                                error = %err,
                                "Failed to read system D-Bus service file"
                            );
                        }
                    }
                }
                if let Some(exec_bin) = read_desktop_exec_bin(candidate) {
                    terminate_unhooked_processes(app_id, &exec_bin);
                }
            } else if let Err(err) = remove_file_if_exists(&user_service) {
                tracing::warn!(
                    path = %user_service.display(),
                    error = %err,
                    "Failed to remove user D-Bus service override"
                );
            }
        }
    }

    let apps_dir = user_applications_dir();
    if let Err(err) = Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status()
    {
        tracing::debug!(
            path = %apps_dir.display(),
            error = %err,
            "update-desktop-database unavailable or failed"
        );
    }

    reload_dbus_session_config();
}

fn reload_dbus_session_config() {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            tracing::debug!(error = %err, "Failed to build Tokio runtime for D-Bus ReloadConfig");
            return;
        }
    };
    rt.block_on(async {
        match zbus::Connection::session().await {
            Ok(conn) => {
                if let Err(err) = conn
                    .call_method(
                        Some("org.freedesktop.DBus"),
                        "/org/freedesktop/DBus",
                        Some("org.freedesktop.DBus"),
                        "ReloadConfig",
                        &(),
                    )
                    .await
                {
                    tracing::debug!(error = %err, "Failed to reload D-Bus session config");
                }
            }
            Err(err) => {
                tracing::debug!(error = %err, "Session D-Bus unavailable while reloading config");
            }
        }
    });
}

fn inject_env_into_dbus_service(
    raw_service: &str,
    inject_env: &BTreeMap<String, String>,
) -> Option<String> {
    if inject_env.is_empty() {
        return None;
    }
    let mut env_pairs = Vec::new();
    for (k, v) in inject_env {
        let expanded = expand_tokens(v);
        env_pairs.push(format!("{k}={expanded}"));
    }
    env_pairs.sort();
    let env_prefix = format!("/usr/bin/env {} ", env_pairs.join(" "));

    let mut out_lines = Vec::new();
    for line in raw_service.lines() {
        // Strip `SystemdService=` so `dbus-broker` executes our patched `Exec=` line directly
        // instead of delegating activation to an unhooked systemd user unit (e.g. `gnome-software.service`).
        if line.trim_start().starts_with("SystemdService=") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Exec=") {
            let clean_rest = rest.trim();
            if !clean_rest.contains("LD_PRELOAD=") {
                out_lines.push(format!("Exec={env_prefix}{clean_rest}"));
                continue;
            }
        }
        out_lines.push(line.to_string());
    }
    Some(out_lines.join("\n") + "\n")
}
