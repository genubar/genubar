//! Zero-dependency IPC constants, action contracts, and shared hook identifiers for Genubar.

use std::ffi::CStr;

// D-Bus service names, interfaces, and object paths
pub const TRACKER_BUS_NAME: &str = "org.genubar.WindowTracker";
pub const TRACKER_OBJECT_PATH: &str = "/org/genubar/WindowTracker";
pub const TRACKER_INTERFACE: &str = "org.genubar.WindowTracker";

pub const TRACKER_BUS_NAME_C: &CStr = c"org.genubar.WindowTracker";
pub const TRACKER_OBJECT_PATH_C: &CStr = c"/org/genubar/WindowTracker";
pub const TRACKER_INTERFACE_C: &CStr = c"org.genubar.WindowTracker";

pub const REGISTRAR_BUS_NAME: &str = "com.canonical.AppMenu.Registrar";
pub const REGISTRAR_OBJECT_PATH: &str = "/com/canonical/AppMenu/Registrar";

pub const GTK4_BRIDGE_INTERFACE: &str = "org.genubar.Gtk4Bridge";
pub const GTK4_BRIDGE_PATH: &str = "/org/genubar/gtk4_bridge";
pub const GTK4_MENU_PATH: &str = "/org/genubar/gtk4_menu";
pub const GTK4_APP_PATH: &str = "/org/genubar/gtk4_app";
pub const GTK4_WIN_PATH: &str = "/org/genubar/gtk4_win";

pub const GTK4_BRIDGE_INTERFACE_C: &CStr = c"org.genubar.Gtk4Bridge";
pub const GTK4_BRIDGE_PATH_C: &CStr = c"/org/genubar/gtk4_bridge";
pub const GTK4_MENU_PATH_C: &CStr = c"/org/genubar/gtk4_menu";
pub const GTK4_APP_PATH_C: &CStr = c"/org/genubar/gtk4_app";
pub const GTK4_WIN_PATH_C: &CStr = c"/org/genubar/gtk4_win";

pub const HIDDEN_BUTTONS_ENV: &str = "GENUBAR_GTK4_HIDE_BUTTONS";
pub const HIDDEN_BUTTONS_ENV_C: &CStr = c"GENUBAR_GTK4_HIDE_BUTTONS";

// Cross-process custom action contracts (shared between genubar-dbus and genubar-gtk4-hook)
pub const ACTION_THEME_SYSTEM: &str = "genubar.theme-system";
pub const ACTION_THEME_LIGHT: &str = "genubar.theme-light";
pub const ACTION_THEME_DARK: &str = "genubar.theme-dark";

pub const ACTION_ZOOM_IN: &str = "genubar.zoom-in";
pub const ACTION_ZOOM_OUT: &str = "genubar.zoom-out";
pub const ACTION_ZOOM_RESET: &str = "genubar.zoom-reset";

// GNOME Shell extension metadata
pub const EXTENSION_UUID: &str = "genubar@github.com";
pub const LEGACY_EXTENSION_UUID: &str = "genubar@local";
pub const EXTENSION_FILES: &[&str] = &[
    "metadata.json",
    "extension.js",
    "stylesheet.css",
    "lib/core/l0Cache.js",
    "lib/core/menuModel.js",
    "lib/core/menuStore.js",
    "lib/core/slotLayout.js",
    "lib/core/textUtils.js",
    "lib/ipc/dbusClient.js",
    "lib/ipc/versionCheck.js",
    "lib/platform/actionRegistry.js",
    "lib/platform/focusTracker.js",
    "lib/platform/panelAdapter.js",
    "lib/platform/shortcuts.js",
    "lib/platform/themeManager.js",
    "lib/platform/windowActions.js",
    "lib/ui/cascadeController.js",
    "lib/ui/iconCache.js",
    "lib/ui/menuBar.js",
    "lib/ui/menuButton.js",
    "lib/ui/menuColumn.js",
    "lib/ui/menuRow.js",
    "lib/ui/searchWidget.js",
    "lib/ui/windowListExpander.js",
];

// Shared GTK4 hook library markers & protected process lists
pub const GTK4_HOOK_ACTIVE_SO_NAME: &str = "active_gtk4_hook.so";
pub const GTK4_HOOK_CANONICAL_SO_NAME: &str = "libgenubar_gtk4_hook.so";
pub const GTK4_HOOK_LEGACY_SO_NAME: &str = "libmenu_gtk4_hook.so";

pub const GTK4_HOOK_SO_NAMES: [&str; 3] = [
    GTK4_HOOK_ACTIVE_SO_NAME,
    GTK4_HOOK_CANONICAL_SO_NAME,
    GTK4_HOOK_LEGACY_SO_NAME,
];

pub const GTK4_HOOK_MAP_MARKERS: [&str; 3] = [
    "genubar_gtk4_hook",
    "menu_gtk4_hook",
    "active_gtk4_hook",
];

pub const PROTECTED_SHELL_BINARIES: [&str; 7] = [
    "ptyxis",
    "gnome-terminal",
    "gnome-terminal-server",
    "bash",
    "zsh",
    "fish",
    "sh",
];

pub const PROTECTED_HOOK_GUARD_BINARIES: [&str; 12] = [
    "ptyxis",
    "gnome-terminal",
    "gnome-terminal-server",
    "bash",
    "zsh",
    "fish",
    "sh",
    "gapplication",
    "genubar-settings",
    "genubar-cli",
    "genubar-daemon",
    "genubar-installer",
];

pub const PROTECTED_HOOK_LAUNCHERS: [&str; 6] = [
    "flatpak",
    "rustrover",
    "rustrover.sh",
    "vscode",
    "code",
    "code-insiders",
];

/// Returns true when a `/proc/<pid>/maps` text contains any Genubar GTK4 hook `.so` mapping.
pub fn maps_contain_gtk4_hook(maps: &str) -> bool {
    GTK4_HOOK_MAP_MARKERS
        .iter()
        .any(|marker| maps.contains(marker))
}

/// Returns true if an executable basename is a terminal/shell that must never be killed.
pub fn is_protected_shell_binary(exe_name: &str) -> bool {
    PROTECTED_SHELL_BINARIES.contains(&exe_name)
}

/// Returns true if `exe_name` must never be terminated by the single-instance hook guard.
pub fn is_protected_for_hook_guard(exe_name: &str) -> bool {
    exe_name.is_empty() || PROTECTED_HOOK_GUARD_BINARIES.contains(&exe_name)
}

/// Returns true if `cmd_or_bin` launches a terminal, shell, Genubar binary, or IDE/container
/// that must NOT receive `LD_PRELOAD=libgenubar_gtk4_hook.so`.
pub fn is_protected_from_gtk4_hook(cmd_or_bin: &str) -> bool {
    let trimmed = cmd_or_bin.trim();
    if trimmed.is_empty() {
        return true;
    }

    for segment in trimmed.split(&[';', '|', '&'][..]) {
        let mut iter = segment
            .split_whitespace()
            .map(|t| t.trim_matches(&['"', '\''][..]))
            .filter(|t| !t.is_empty());

        let mut exe_token = None;
        for tok in iter.by_ref() {
            let base = tok.rsplit('/').next().unwrap_or(tok).to_lowercase();
            if base == "env" || tok.contains('=') {
                continue;
            }
            exe_token = Some(base);
            break;
        }

        if let Some(base) = exe_token
            && (PROTECTED_HOOK_GUARD_BINARIES.contains(&base.as_str())
                || PROTECTED_HOOK_LAUNCHERS.contains(&base.as_str())
                || base == "genubar"
                || base.starts_with("genubar-"))
            {
                return true;
            }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_cstrs_match_utf8_constants() {
        assert_eq!(TRACKER_BUS_NAME_C.to_str().unwrap(), TRACKER_BUS_NAME);
        assert_eq!(TRACKER_OBJECT_PATH_C.to_str().unwrap(), TRACKER_OBJECT_PATH);
        assert_eq!(GTK4_BRIDGE_INTERFACE_C.to_str().unwrap(), GTK4_BRIDGE_INTERFACE);
        assert_eq!(GTK4_BRIDGE_PATH_C.to_str().unwrap(), GTK4_BRIDGE_PATH);
        assert_eq!(GTK4_MENU_PATH_C.to_str().unwrap(), GTK4_MENU_PATH);
        assert_eq!(GTK4_APP_PATH_C.to_str().unwrap(), GTK4_APP_PATH);
        assert_eq!(GTK4_WIN_PATH_C.to_str().unwrap(), GTK4_WIN_PATH);
        assert_eq!(HIDDEN_BUTTONS_ENV_C.to_str().unwrap(), HIDDEN_BUTTONS_ENV);
    }

    #[test]
    fn hook_markers_and_protected_binaries_work() {
        assert!(maps_contain_gtk4_hook("7f00-7f01 r-xp /home/u/.local/lib/genubar/active_gtk4_hook.so"));
        assert!(maps_contain_gtk4_hook("libgenubar_gtk4_hook.so"));
        assert!(!maps_contain_gtk4_hook("libgtk-4.so.1"));

        assert!(is_protected_shell_binary("ptyxis"));
        assert!(!is_protected_shell_binary("org.gnome.Calendar"));
        assert!(is_protected_for_hook_guard("genubar-settings"));
        assert!(is_protected_for_hook_guard(""));
        assert!(!is_protected_for_hook_guard("gnome-calendar"));

        assert!(is_protected_from_gtk4_hook("ptyxis --new-window"));
        assert!(is_protected_from_gtk4_hook("/usr/bin/env FOO=1 bash -lc id"));
        assert!(is_protected_from_gtk4_hook("flatpak run org.gnome.Loupe"));
        assert!(is_protected_from_gtk4_hook("/usr/bin/code --new-window"));
        assert!(is_protected_from_gtk4_hook("genubar-settings"));
        assert!(!is_protected_from_gtk4_hook("gnome-text-editor ~/genubar/readme.md"));
        assert!(!is_protected_from_gtk4_hook("nautilus"));
    }
}
