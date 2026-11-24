//! Self-healing guard against stale unhooked `--gapplication-service` background processes.

use crate::hook_log;
use genubar_protocol::{is_protected_for_hook_guard, maps_contain_gtk4_hook};
use std::time::Duration;

fn extract_script_identity(pid_dir: &str) -> Option<String> {
    let raw = std::fs::read(format!("{pid_dir}/cmdline")).ok()?;
    let args: Vec<String> = raw
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect();
    args.iter().skip(1).find(|a| !a.starts_with('-')).cloned()
}

/// Terminates any unhooked sibling process of the same executable before `g_application_run`
/// so single-instance GApplications (`Calendar`, `Clocks`, `Software`, `Weather`, etc.) become
/// the primary instance under this hooked process rather than forwarding `Activate` and exiting.
pub fn terminate_stale_unhooked_instances() {
    let self_pid = unsafe { libc::getpid() } as u32;
    let Ok(self_exe) = std::fs::read_link("/proc/self/exe") else {
        return;
    };
    let exe_name = self_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    // Never terminate shells, terminal emulators, or Genubar binaries
    if is_protected_for_hook_guard(exe_name) {
        return;
    }

    let is_interpreter = exe_name.starts_with("python") || exe_name.starts_with("gjs");
    let self_script = if is_interpreter {
        extract_script_identity("/proc/self")
    } else {
        None
    };
    if is_interpreter && self_script.is_none() {
        return;
    }

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return;
    };

    let mut killed_pids = Vec::new();
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid <= 1 || pid == self_pid {
            continue;
        }

        let proc_dir = format!("/proc/{pid}");
        let Ok(other_exe) = std::fs::read_link(format!("{proc_dir}/exe")) else {
            continue;
        };
        if other_exe != self_exe {
            continue;
        }
        if is_interpreter && extract_script_identity(&proc_dir) != self_script {
            continue;
        }

        let maps = std::fs::read_to_string(format!("{proc_dir}/maps")).unwrap_or_default();
        if !maps.is_empty() && !maps_contain_gtk4_hook(&maps) {
            hook_log!(
                "Terminating stale unhooked background instance of '{exe_name}' (PID={pid}) so hooked instance takes over D-Bus name"
            );
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            killed_pids.push(pid);
        }
    }

    if !killed_pids.is_empty() {
        for _ in 0..30 {
            let any_alive = killed_pids
                .iter()
                .any(|pid| std::path::Path::new(&format!("/proc/{pid}")).exists());
            if !any_alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
