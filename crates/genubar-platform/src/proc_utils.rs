use genubar_core::MenuError;
use genubar_protocol::{is_protected_shell_binary, maps_contain_gtk4_hook};
use std::fs;

/// Checks `/proc/<pid>/maps` if readable and non-empty. Returns `Some(true)` if an injected
/// Genubar GTK4 hook library is mapped, `Some(false)` if absent, or `None` if unreadable/empty.
pub fn try_check_pid_gtk4_hooked(pid: u32) -> Option<bool> {
    if pid == 0 {
        return Some(false);
    }
    let maps = fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    if maps.is_empty() {
        return None;
    }
    Some(maps_contain_gtk4_hook(&maps))
}

/// Returns true if `/proc/<pid>/maps` contains an injected Genubar GTK4 hook library.
pub fn is_pid_gtk4_hooked(pid: u32) -> bool {
    try_check_pid_gtk4_hooked(pid).unwrap_or(false)
}

/// Parses PPID from the raw contents of `/proc/<pid>/stat`.
pub fn parse_stat_ppid(stat_raw: &str) -> Option<u32> {
    let after_comm = stat_raw.rfind(')')?;
    let mut fields = stat_raw[after_comm + 1..].split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse::<u32>().ok()
}

/// Reads parent PID (PPID) from `/proc/<pid>/stat`.
pub fn get_parent_pid(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat_ppid(&stat)
}

/// Identifies the application binary name from `/proc/<pid>/comm` (lowercased, fallback `"unknown"`).
pub fn get_process_comm(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .unwrap_or_else(|_| String::from("unknown"))
        .trim()
        .to_lowercase()
}

/// Reads `/proc/<pid>/cmdline` with NUL bytes replaced by spaces.
pub fn read_proc_cmdline(pid: u32) -> String {
    fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| {
            b.into_iter()
                .map(|c| if c == 0 { b' ' } else { c })
                .collect::<Vec<u8>>()
        })
        .map(|v| String::from_utf8_lossy(&v).to_string())
        .unwrap_or_default()
}

/// Matches a window PID and a D-Bus peer PID only when they share the exact binary `comm` name.
pub fn pids_match(window_pid: u32, bus_pid: u32) -> bool {
    if window_pid == 0 || bus_pid == 0 {
        return false;
    }
    if window_pid == bus_pid {
        return true;
    }

    let comm_win = get_process_comm(window_pid);
    let comm_bus = get_process_comm(bus_pid);
    if comm_win == "unknown" || comm_bus == "unknown" {
        return false;
    }

    if comm_win == comm_bus {
        return true;
    }

    let p1 = get_parent_pid(window_pid);
    let p2 = get_parent_pid(bus_pid);
    if p1 == Some(bus_pid) || p2 == Some(window_pid) {
        let is_session_launcher = matches!(
            comm_bus.as_str(),
            "systemd" | "gnome-shell" | "init" | "dbus-daemon"
        ) || matches!(
            comm_win.as_str(),
            "systemd" | "gnome-shell" | "init" | "dbus-daemon"
        );
        return !is_session_launcher && comm_win == comm_bus;
    }

    false
}

/// Sends `SIGTERM` to `pid`, returning an error if `pid` is invalid or `kill(2)` fails.
pub fn terminate_pid(pid: u32) -> Result<(), MenuError> {
    if pid == 0 {
        return Err(MenuError::Config(String::from(
            "Refusing to terminate PID 0 (process group)",
        )));
    }
    let raw_pid = libc::pid_t::try_from(pid)
        .map_err(|_| MenuError::Config(format!("PID {pid} exceeds pid_t range")))?;
    let rc = unsafe { libc::kill(raw_pid, libc::SIGTERM) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        return Err(MenuError::IoPermission(format!(
            "Failed to send SIGTERM to PID {pid}: {err}"
        )));
    }
    Ok(())
}

/// Terminates any running process matching `app_id` or `exec_bin` without `libgenubar_gtk4_hook.so` loaded.
pub fn terminate_unhooked_processes(app_id: &str, exec_bin: &str) {
    if exec_bin.is_empty() || is_protected_shell_binary(exec_bin) {
        return;
    }

    let Ok(entries) = fs::read_dir("/proc") else {
        return;
    };
    let self_pid = std::process::id();

    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid <= 1 || pid == self_pid {
            continue;
        }

        let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let cmdline = read_proc_cmdline(pid);

        let prefix_len = exec_bin.len().min(15);
        let comm_matches =
            !comm.is_empty() && exec_bin != "gapplication" && comm == exec_bin[..prefix_len];
        let script_matches = !app_id.is_empty()
            && cmdline.contains(app_id)
            && !cmdline.contains("BackgroundService")
            && !cmdline.contains("/usr/bin/gnome-shell ");

        if (comm_matches || script_matches)
            && try_check_pid_gtk4_hooked(pid) == Some(false)
            && let Err(err) = terminate_pid(pid)
        {
            tracing::debug!(pid, error = %err, "Failed terminating unhooked process");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_ppid_and_handles_zero_pid() {
        assert!(!is_pid_gtk4_hooked(0));
        assert_eq!(try_check_pid_gtk4_hooked(0), Some(false));
        assert!(terminate_pid(0).is_err());
        assert_eq!(parse_stat_ppid("1234 (my (app)) S 42 100 200"), Some(42));
    }
}
