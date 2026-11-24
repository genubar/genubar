use crate::paths::{desktop_search_dirs, flatpak_app_dirs, user_dbus_services_dir};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Checks if a binary executable exists in any `$PATH` directory.
pub fn has_binary_in_path(binary: &str) -> bool {
    let Ok(path_var) = env::var("PATH") else {
        return false;
    };
    path_var
        .split(':')
        .any(|dir| Path::new(dir).join(binary).is_file())
}

pub fn any_binary_in_path(binaries: &[String]) -> bool {
    binaries.iter().any(|b| has_binary_in_path(b))
}

/// Checks if a Flatpak application ID exists under user or system Flatpak installations.
pub fn has_flatpak_app(app_id: &str) -> bool {
    flatpak_app_dirs()
        .iter()
        .any(|dir| dir.join(app_id).exists())
}

pub fn any_flatpak_app(app_ids: &[String]) -> bool {
    app_ids.iter().any(|id| has_flatpak_app(id))
}

/// Finds the first matching `.desktop` file across standard system/Flatpak/user application directories.
pub fn locate_desktop_file(candidates: &[&str]) -> Option<PathBuf> {
    let dirs = desktop_search_dirs();
    for name in candidates {
        if let Some(found) = dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            return Some(found);
        }
    }
    None
}

pub fn has_any_desktop_file(desktop_files: &[String]) -> bool {
    let dirs = desktop_search_dirs();
    desktop_files
        .iter()
        .any(|df| dirs.iter().any(|d| d.join(df).is_file()))
}

/// Extracts the binary basename from `Exec=` in `/usr/share/applications/<desktop_file>` (or located file).
pub fn read_desktop_exec_bin(desktop_file: &str) -> Option<String> {
    let path = locate_desktop_file(&[desktop_file])?;
    let raw = fs::read_to_string(path).ok()?;
    parse_desktop_exec_bin(&raw)
}

pub fn parse_desktop_exec_bin(raw_desktop: &str) -> Option<String> {
    raw_desktop.lines().find_map(|line| {
        let rest = line.strip_prefix("Exec=")?;
        let first_token = rest.split_whitespace().next()?;
        Path::new(first_token)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
    })
}

pub fn system_dbus_service_path(app_id: &str) -> PathBuf {
    PathBuf::from(format!("/usr/share/dbus-1/services/{app_id}.service"))
}

pub fn user_dbus_service_path(app_id: &str) -> PathBuf {
    user_dbus_services_dir().join(format!("{app_id}.service"))
}

/// Resolves the underlying `Exec=` command for a D-Bus activatable `app_id` service.
pub fn read_dbus_service_exec(app_id: &str) -> Option<String> {
    let raw_service = fs::read_to_string(system_dbus_service_path(app_id)).ok()?;
    raw_service.lines().find_map(|line| {
        let exec_val = line.strip_prefix("Exec=")?;
        let bin_cmd = exec_val
            .replace("--gapplication-service", "")
            .trim()
            .to_string();
        (!bin_cmd.is_empty()).then_some(bin_cmd)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_exec_basename() {
        let desktop =
            "[Desktop Entry]\nName=App\nExec=/usr/bin/gnome-calendar --gapplication-service\n";
        assert_eq!(
            parse_desktop_exec_bin(desktop).as_deref(),
            Some("gnome-calendar")
        );
    }
}
