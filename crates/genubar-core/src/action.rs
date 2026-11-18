use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PatchErrorPolicy {
    #[default]
    Strict,
    IgnoreSystemErrors,
    IgnoreAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemCommand {
    QuitApp,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    FullscreenWindow,
    AlwaysOnTop,
    MoveWorkspaceLeft,
    MoveWorkspaceRight,
    CenterWindow,
    /// Text case transformation commands used by the standard Edit menu.
    TransformUpper,
    TransformLower,
    TransformCapitalize,
    RefreshMenu,
    ReloadConfig,
    CaptureMenuSnapshot,
    LockScreen,
    Logout,
}

impl SystemCommand {
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "quit_app" => Some(Self::QuitApp),
            "close_window" => Some(Self::CloseWindow),
            "minimize_window" => Some(Self::MinimizeWindow),
            "maximize_window" => Some(Self::MaximizeWindow),
            "fullscreen_window" => Some(Self::FullscreenWindow),
            "always_on_top" => Some(Self::AlwaysOnTop),
            "move_workspace_left" => Some(Self::MoveWorkspaceLeft),
            "move_workspace_right" => Some(Self::MoveWorkspaceRight),
            "center_window" => Some(Self::CenterWindow),
            "transform_upper" => Some(Self::TransformUpper),
            "transform_lower" => Some(Self::TransformLower),
            "transform_capitalize" => Some(Self::TransformCapitalize),
            "refresh_menu" => Some(Self::RefreshMenu),
            "reload_config" => Some(Self::ReloadConfig),
            "capture_menu_snapshot" => Some(Self::CaptureMenuSnapshot),
            "lock_screen" => Some(Self::LockScreen),
            "logout" => Some(Self::Logout),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum CustomAction {
    Command(String),
    Shell(String),
    OpenUri(String),
    System(SystemCommand),
    CopyText(String),
    InsertText(String),
    /// Synthesizes a keyboard shortcut against the focused window (e.g. `key:Ctrl+Z`).
    KeyShortcut(String),
    DbusCall {
        dest: String,
        path: String,
        interface: String,
        method: String,
    },
    GtkAction(String),
}

impl CustomAction {
    pub fn parse(action_str: &str) -> Option<Self> {
        let (prefix, payload) = action_str.split_once(':')?;
        let trimmed = payload.trim();
        match prefix {
            "cmd" => Some(Self::Command(trimmed.to_string())),
            "sh" => Some(Self::Shell(trimmed.to_string())),
            "uri" => Some(Self::OpenUri(trimmed.to_string())),
            "sys" => SystemCommand::from_slug(trimmed).map(Self::System),
            "copy" => Some(Self::CopyText(payload.to_string())),
            "insert" => Some(Self::InsertText(payload.to_string())),
            "key" => Some(Self::KeyShortcut(trimmed.to_string())),
            "dbus" => {
                let parts: Vec<&str> = trimmed.splitn(4, '|').map(str::trim).collect();
                if parts.len() == 4 {
                    Some(Self::DbusCall {
                        dest: parts[0].to_string(),
                        path: parts[1].to_string(),
                        interface: parts[2].to_string(),
                        method: parts[3].to_string(),
                    })
                } else {
                    None
                }
            }
            "gtk" => Some(Self::GtkAction(trimmed.to_string())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_supported_custom_action_prefixes_and_rejects_malformed() {
        assert_eq!(
            CustomAction::parse("cmd: nautilus --new-window"),
            Some(CustomAction::Command("nautilus --new-window".to_string()))
        );
        assert_eq!(
            CustomAction::parse("sh: echo hi | cat"),
            Some(CustomAction::Shell("echo hi | cat".to_string()))
        );
        assert_eq!(
            CustomAction::parse("uri:~/Downloads"),
            Some(CustomAction::OpenUri("~/Downloads".to_string()))
        );
        assert_eq!(
            CustomAction::parse("sys:capture_menu_snapshot"),
            Some(CustomAction::System(SystemCommand::CaptureMenuSnapshot))
        );
        assert_eq!(
            CustomAction::parse("copy:  preserve spaces "),
            Some(CustomAction::CopyText("  preserve spaces ".to_string()))
        );
        assert_eq!(
            CustomAction::parse("insert:Hello"),
            Some(CustomAction::InsertText("Hello".to_string()))
        );
        assert_eq!(
            CustomAction::parse("key:Ctrl+Shift+V"),
            Some(CustomAction::KeyShortcut("Ctrl+Shift+V".to_string()))
        );
        assert_eq!(
            CustomAction::parse("gtk:app.quit"),
            Some(CustomAction::GtkAction("app.quit".to_string()))
        );
        assert_eq!(
            CustomAction::parse("dbus:org.foo|/org/foo|org.foo.Iface|Ping"),
            Some(CustomAction::DbusCall {
                dest: "org.foo".to_string(),
                path: "/org/foo".to_string(),
                interface: "org.foo.Iface".to_string(),
                method: "Ping".to_string(),
            })
        );
        assert_eq!(CustomAction::parse("dbus:only|two|parts"), None);
        assert_eq!(CustomAction::parse("sys:nonexistent_command"), None);
        assert_eq!(CustomAction::parse("unknown:value"), None);
        assert_eq!(CustomAction::parse("missing_colon"), None);
    }
}