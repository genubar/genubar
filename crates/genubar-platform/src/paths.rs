use std::env;
use std::path::{Path, PathBuf};

pub const APP_ICON_NAME: &str = "org.genubar.Settings";
pub const INSTALLER_ICON_NAME: &str = "org.genubar.Installer";
pub const EMBEDDED_APP_ICON_SVG: &str = include_str!("../../../docs/assets/icon.svg");

/// Returns the user's home directory from `$HOME`, falling back to `/home`.
pub fn home_dir() -> PathBuf {
    env::var("HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home"))
}

pub fn config_dir() -> PathBuf {
    home_dir().join(".config/genubar")
}

pub fn data_dir() -> PathBuf {
    home_dir().join(".local/share/genubar")
}

pub fn lib_dir() -> PathBuf {
    home_dir().join(".local/lib/genubar")
}

pub fn bin_dir() -> PathBuf {
    home_dir().join(".local/bin")
}

pub fn cache_dir() -> PathBuf {
    home_dir().join(".cache/genubar")
}

pub fn extensions_dir() -> PathBuf {
    home_dir().join(".local/share/gnome-shell/extensions")
}

pub fn systemd_user_dir() -> PathBuf {
    home_dir().join(".config/systemd/user")
}

pub fn user_applications_dir() -> PathBuf {
    home_dir().join(".local/share/applications")
}

pub fn user_icons_scalable_apps_dir() -> PathBuf {
    home_dir().join(".local/share/icons/hicolor/scalable/apps")
}

pub fn user_dbus_services_dir() -> PathBuf {
    home_dir().join(".local/share/dbus-1/services")
}

pub fn flatpak_app_dirs() -> [PathBuf; 2] {
    [
        home_dir().join(".local/share/flatpak/app"),
        PathBuf::from("/var/lib/flatpak/app"),
    ]
}

pub fn desktop_search_dirs() -> [PathBuf; 5] {
    let home = home_dir();
    [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
        home.join(".local/share/applications"),
    ]
}

/// Expands leading `~`, `~/`, `$HOME`, or `$HOME/` to [`home_dir()`].
pub fn expand_tilde(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" || trimmed == "$HOME" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("$HOME/") {
        return home_dir().join(rest);
    }
    PathBuf::from(trimmed)
}

/// Resolves a user-configured path: expands `~`/`$HOME` prefixes, preserves absolute paths,
/// and joins relative paths onto `base_dir`.
pub fn resolve_user_path(base_dir: &Path, target: &Path) -> PathBuf {
    let s = target.to_string_lossy();
    if s == "~" || s.starts_with("~/") || s == "$HOME" || s.starts_with("$HOME/") {
        return expand_tilde(&s);
    }
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        base_dir.join(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_user_paths_and_tilde_consistently() {
        let base = Path::new("/tmp/genubar-cfg");
        let home = home_dir();
        assert_eq!(expand_tilde("~/styles/test.css"), home.join("styles/test.css"));
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("$HOME"), home);
        assert_eq!(
            resolve_user_path(base, Path::new("$HOME/.local/lib")),
            home.join(".local/lib")
        );
        assert_eq!(
            resolve_user_path(base, Path::new("styles/custom.css")),
            base.join("styles/custom.css")
        );
        assert_eq!(
            resolve_user_path(base, Path::new("/etc/genubar.toml")),
            PathBuf::from("/etc/genubar.toml")
        );
        assert_eq!(desktop_search_dirs().len(), 5);
    }
}
