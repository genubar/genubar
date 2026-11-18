use genubar_core::{ActiveWindowInfo, SystemCommand};
use std::process::Stdio;
use tokio::process::Command;
use zbus::Connection;

pub struct ActionExecutor;

impl ActionExecutor {
    pub fn expand_window_vars(raw: &str, info: &ActiveWindowInfo) -> String {
        raw.replace("{pid}", &info.pid.to_string())
            .replace("{xid}", &info.xid.to_string())
            .replace("{app_id}", &info.app_id)
            .replace("{title}", &info.title)
    }

    fn resolve_gtk4_hook_so() -> Option<String> {
        genubar_platform::resolve_installed_gtk4_hook_so()
    }

    pub fn should_inject_gtk4_hook(cmd_str: &str) -> bool {
        !genubar_protocol::is_protected_from_gtk4_hook(cmd_str)
    }

    fn attach_window_env(cmd: &mut Command, info: &ActiveWindowInfo, cmd_str: &str) {
        cmd.env("GENUBAR_PID", info.pid.to_string())
            .env("GENUBAR_XID", info.xid.to_string())
            .env("GENUBAR_APP_ID", &info.app_id)
            .env("GENUBAR_TITLE", &info.title);

        if Self::should_inject_gtk4_hook(cmd_str)
            && let Some(so_path) = Self::resolve_gtk4_hook_so() {
                cmd.env("LD_PRELOAD", so_path);
            }
    }

    fn spawn_detached(mut cmd: Command, label: &str) -> bool {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(_child) => true,
            Err(err) => {
                tracing::warn!(command = label, error = %err, "Failed to spawn detached process");
                false
            }
        }
    }

    pub fn resolve_program_path(program: &str) -> std::path::PathBuf {
        if !program.contains('/') {
            let local_cand = genubar_platform::bin_dir().join(program);
            if local_cand.is_file() {
                return local_cand;
            }
        }
        std::path::PathBuf::from(program)
    }

    pub fn spawn_command(cmd_line: &str, info: &ActiveWindowInfo) -> bool {
        let expanded = Self::expand_window_vars(cmd_line, info);
        let mut parts = expanded.split_whitespace();
        if let Some(program) = parts.next() {
            let args: Vec<&str> = parts.collect();
            let program_path = Self::resolve_program_path(program);
            let mut cmd = Command::new(&program_path);
            cmd.args(args);
            Self::attach_window_env(&mut cmd, info, &expanded);
            return Self::spawn_detached(cmd, program);
        }
        false
    }

    pub fn spawn_shell(script: &str, info: &ActiveWindowInfo) -> bool {
        let expanded = Self::expand_window_vars(script, info);
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&expanded);
        Self::attach_window_env(&mut cmd, info, &expanded);
        Self::spawn_detached(cmd, "sh -c")
    }

    pub fn open_uri(uri: &str, info: &ActiveWindowInfo) -> bool {
        let with_vars = Self::expand_window_vars(uri, info);
        let expanded = genubar_platform::expand_tilde(&with_vars)
            .to_string_lossy()
            .to_string();
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&expanded);
        Self::attach_window_env(&mut cmd, info, "xdg-open");
        Self::spawn_detached(cmd, "xdg-open")
    }

    pub async fn call_dbus(
        conn: &Connection,
        dest: &str,
        path: &str,
        interface: &str,
        method: &str,
    ) -> bool {
        if let Err(err) = conn
            .call_method(Some(dest), path, Some(interface), method, &())
            .await
        {
            tracing::warn!(dest, path, interface, method, error = %err, "Custom D-Bus call failed");
            return false;
        }
        true
    }

    pub fn execute_os_system_command(cmd: SystemCommand, active_pid: u32) -> Option<bool> {
        match cmd {
            SystemCommand::QuitApp => {
                if active_pid > 0 {
                    match genubar_platform::terminate_pid(active_pid) {
                        Ok(()) => Some(true),
                        Err(err) => {
                            tracing::warn!(
                                pid = active_pid,
                                error = %err,
                                "Failed terminating focused application"
                            );
                            Some(false)
                        }
                    }
                } else {
                    Some(false)
                }
            }
            SystemCommand::LockScreen => {
                let mut c = Command::new("loginctl");
                c.arg("lock-session");
                Some(Self::spawn_detached(c, "loginctl lock-session"))
            }
            SystemCommand::Logout => {
                let mut c = Command::new("gnome-session-quit");
                c.arg("--logout");
                Some(Self::spawn_detached(c, "gnome-session-quit"))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_window_variables_and_filters_protected_hook_targets() {
        let info = ActiveWindowInfo {
            pid: 4242,
            xid: 99,
            app_id: String::from("org.gnome.Nautilus"),
            title: String::from("Home"),
            ..Default::default()
        };
        assert_eq!(
            ActionExecutor::expand_window_vars("tool --pid={pid} --app={app_id}", &info),
            "tool --pid=4242 --app=org.gnome.Nautilus"
        );
        assert!(ActionExecutor::should_inject_gtk4_hook("nautilus"));
        assert!(ActionExecutor::should_inject_gtk4_hook(
            "gnome-text-editor ~/genubar/notes.txt"
        ));
        assert!(!ActionExecutor::should_inject_gtk4_hook("ptyxis --new-window"));
        assert!(!ActionExecutor::should_inject_gtk4_hook("/usr/bin/bash -lc echo"));
        assert!(!ActionExecutor::should_inject_gtk4_hook("genubar-settings"));
        assert!(!ActionExecutor::should_inject_gtk4_hook("/usr/bin/code --new-window"));
        assert!(!ActionExecutor::should_inject_gtk4_hook("flatpak run org.gnome.Loupe"));
        assert_eq!(
            ActionExecutor::resolve_program_path("/usr/bin/genubar-settings"),
            std::path::PathBuf::from("/usr/bin/genubar-settings")
        );
        let resolved_missing = ActionExecutor::resolve_program_path("definitely-not-installed-genubar-bin");
        assert_eq!(
            resolved_missing,
            std::path::PathBuf::from("definitely-not-installed-genubar-bin")
        );
    }
}
