use genubar_core::MenuError;
use genubar_platform::fs_utils::{ensure_dir, write_text_file as write_string};
use std::path::Path;

pub const DEFAULT_SETTINGS_TOML: &str = include_str!("../defaults/settings.toml");
pub const DEFAULT_ICONS_TOML: &str = include_str!("../defaults/icons.toml");
pub const DEFAULT_SAMPLE_PACK_TOML: &str = include_str!("../defaults/sample_pack.toml");
pub const DEFAULT_DESKTOP_TOML: &str = include_str!("../defaults/desktop_menu.toml");
pub const DEFAULT_APP_MENU_TOML: &str = include_str!("../defaults/app_menu.toml");
pub const DEFAULT_STYLE_CSS: &str = include_str!("../defaults/style.css");

pub const DEFAULT_MEDIA_PROFILE_TOML: &str =
    include_str!("../defaults/profiles/media_players.toml");
pub const DEFAULT_FIREFOX_PROFILE_TOML: &str = include_str!("../defaults/profiles/firefox.toml");
pub const DEFAULT_NAUTILUS_PROFILE_TOML: &str = include_str!("../defaults/profiles/nautilus.toml");
pub const DEFAULT_PTYXIS_PROFILE_TOML: &str = include_str!("../defaults/profiles/ptyxis.toml");
pub const DEFAULT_TEXT_EDITOR_PROFILE_TOML: &str =
    include_str!("../defaults/profiles/text_editor.toml");
pub const DEFAULT_LOUPE_PAPERS_PROFILE_TOML: &str =
    include_str!("../defaults/profiles/loupe_papers.toml");

pub const DEFAULT_GNOME_CORE_PATCH_TOML: &str =
    include_str!("../defaults/patches/gnome_core_apps.toml");
pub const DEFAULT_GNOME_CORE_BUNDLE_TOML: &str =
    include_str!("../defaults/bundles/gnome_core_suite/bundle.toml");

pub const DEFAULT_MENU_SCRIPT_PY: &str =
    include_str!("../defaults/scripts/example_menu_transformer.py");
pub const DEFAULT_PATCH_SCRIPT_PY: &str =
    include_str!("../defaults/scripts/example_patch_script.py");
pub const DEFAULT_EXAMPLE_REGISTRY_JSON: &str = include_str!("../defaults/example_registry.json");

pub struct ConfigBootstrapper;

impl ConfigBootstrapper {
    /// Ensures all required directories and default config/bundle files exist in `<base_dir>`.
    pub fn bootstrap_if_missing(base_dir: &Path) -> Result<(), MenuError> {
        Self::create_directory_tree(base_dir)?;
        for (path, content) in Self::default_file_entries(base_dir) {
            if !path.exists() {
                write_string(&path, content)?;
            }
        }
        Ok(())
    }

    /// Overwrites all default configuration files for a full factory reset.
    pub fn overwrite_all_defaults(base_dir: &Path) -> Result<(), MenuError> {
        Self::create_directory_tree(base_dir)?;
        for (path, content) in Self::default_file_entries(base_dir) {
            write_string(&path, content)?;
        }
        Ok(())
    }

    /// Restores only the built-in profile files inside `<base_dir>/profiles/` without touching settings or styles.
    pub fn overwrite_default_profiles(base_dir: &Path) -> Result<(), MenuError> {
        ensure_dir(base_dir.join("profiles"))?;
        for (path, content) in Self::default_profile_entries(base_dir) {
            write_string(&path, content)?;
        }
        Ok(())
    }

    fn default_profile_entries(base_dir: &Path) -> Vec<(std::path::PathBuf, &'static str)> {
        vec![
            (
                base_dir.join("profiles/media_players.toml"),
                DEFAULT_MEDIA_PROFILE_TOML,
            ),
            (
                base_dir.join("profiles/firefox.toml"),
                DEFAULT_FIREFOX_PROFILE_TOML,
            ),
            (
                base_dir.join("profiles/nautilus.toml"),
                DEFAULT_NAUTILUS_PROFILE_TOML,
            ),
            (
                base_dir.join("profiles/ptyxis.toml"),
                DEFAULT_PTYXIS_PROFILE_TOML,
            ),
            (
                base_dir.join("profiles/text_editor.toml"),
                DEFAULT_TEXT_EDITOR_PROFILE_TOML,
            ),
            (
                base_dir.join("profiles/loupe_papers.toml"),
                DEFAULT_LOUPE_PAPERS_PROFILE_TOML,
            ),
        ]
    }

    fn create_directory_tree(base_dir: &Path) -> Result<(), MenuError> {
        let subdirs = [
            "profiles",
            "patches",
            "styles",
            "bundles",
            "bundles/gnome_core_suite",
            "icons",
            "icons/custom_sample",
            "scripts",
            "snapshots",
        ];
        ensure_dir(base_dir)?;
        for sub in subdirs {
            ensure_dir(base_dir.join(sub))?;
        }
        Ok(())
    }

    fn default_file_entries(base_dir: &Path) -> Vec<(std::path::PathBuf, &'static str)> {
        let mut entries = vec![
            (base_dir.join("settings.toml"), DEFAULT_SETTINGS_TOML),
            (base_dir.join("icons.toml"), DEFAULT_ICONS_TOML),
            (
                base_dir.join("icons/custom_sample/pack.toml"),
                DEFAULT_SAMPLE_PACK_TOML,
            ),
            (base_dir.join("desktop_menu.toml"), DEFAULT_DESKTOP_TOML),
            (base_dir.join("app_menu.toml"), DEFAULT_APP_MENU_TOML),
            (base_dir.join("style.css"), DEFAULT_STYLE_CSS),
            (base_dir.join("styles/default.css"), DEFAULT_STYLE_CSS),
            (
                base_dir.join("patches/gnome_core_apps.toml"),
                DEFAULT_GNOME_CORE_PATCH_TOML,
            ),
            (
                base_dir.join("bundles/gnome_core_suite/bundle.toml"),
                DEFAULT_GNOME_CORE_BUNDLE_TOML,
            ),
            (
                base_dir.join("scripts/example_menu_transformer.py"),
                DEFAULT_MENU_SCRIPT_PY,
            ),
            (
                base_dir.join("scripts/example_patch_script.py"),
                DEFAULT_PATCH_SCRIPT_PY,
            ),
            (
                base_dir.join("example_registry.json"),
                DEFAULT_EXAMPLE_REGISTRY_JSON,
            ),
        ];
        entries.extend(Self::default_profile_entries(base_dir));
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleManifest, ConfigManager, IconPackManifest, IconsConfig, ThemeMode};
    use genubar_platform::parse_metadata_block;
    use std::fs;

    #[test]
    fn embedded_defaults_and_bootstrapper_validate_end_to_end() {
        let settings: crate::GlobalSettings = toml::from_str(DEFAULT_SETTINGS_TOML).unwrap();
        let default_settings = crate::GlobalSettings::default();
        assert!(settings.general.show_menu_shortcuts);
        assert_eq!(
            settings.general.show_menu_shortcuts,
            default_settings.general.show_menu_shortcuts
        );
        assert!(!settings.api_tiers.gtk4_hook.hide_original_buttons);
        assert_eq!(
            settings.api_tiers.gtk4_hook.hide_original_buttons,
            default_settings.api_tiers.gtk4_hook.hide_original_buttons
        );
        assert!(
            !settings
                .general
                .registry_urls
                .iter()
                .any(|u| u.contains("example_registry.json"))
        );
        assert_eq!(
            settings.general.registry_urls,
            default_settings.general.registry_urls
        );

        let icons: IconsConfig = toml::from_str(DEFAULT_ICONS_TOML).unwrap();
        assert!(icons.general.show_menu_icons);
        assert_eq!(icons.general, IconsConfig::default().general);
        assert!(!icons.system_rules.is_empty());

        let pack: IconPackManifest = toml::from_str(DEFAULT_SAMPLE_PACK_TOML).unwrap();
        assert_eq!(pack.id, "custom_sample");
        assert!(pack.is_compatible());

        let bundle: BundleManifest = toml::from_str(DEFAULT_GNOME_CORE_BUNDLE_TOML).unwrap();
        assert_eq!(bundle.id, "gnome_core_suite");
        assert!(bundle.is_compatible());
        assert!(!bundle.profiles.is_empty());
        assert!(!bundle.patches.is_empty());

        let menu_script_meta = parse_metadata_block(
            DEFAULT_MENU_SCRIPT_PY,
            "# ==GenubarScript==",
            "# ==/GenubarScript==",
        )
        .expect("menu transformer script metadata");
        assert_eq!(
            menu_script_meta.get("id").map(String::as_str),
            Some("example_menu_transformer")
        );

        let patch_script_meta = parse_metadata_block(
            DEFAULT_PATCH_SCRIPT_PY,
            "# ==GenubarScript==",
            "# ==/GenubarScript==",
        )
        .expect("patch script metadata");
        assert_eq!(
            patch_script_meta.get("id").map(String::as_str),
            Some("example_patch_script")
        );

        assert!(DEFAULT_STYLE_CSS.contains(".genubar-panel-button"));
        assert!(DEFAULT_STYLE_CSS.contains(".genubar-theme-light"));

        let tmp = std::env::temp_dir().join("genubar_test_bootstrap_defaults");
        let _ = fs::remove_dir_all(&tmp);
        ConfigBootstrapper::bootstrap_if_missing(&tmp).unwrap();
        let cfg = ConfigManager::load_from_base_dir(tmp.clone()).unwrap();
        assert_eq!(cfg.settings.style.theme_mode, ThemeMode::Auto);
        assert!(cfg.profiles.len() >= 6);
        assert!(tmp.join("bundles/gnome_core_suite/bundle.toml").is_file());
        assert!(tmp.join("patches/gnome_core_apps.toml").is_file());
        assert!(tmp.join("style.css").is_file());
        assert!(tmp.join("styles/default.css").is_file());
        assert!(tmp.join("icons.toml").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }
}
