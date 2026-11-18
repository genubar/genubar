use crate::defaults::{DEFAULT_SETTINGS_TOML, DEFAULT_STYLE_CSS};
use crate::models::settings::{GlobalSettings, StyleFileConfig};
use crate::utils::toml_io::{parse_embedded_toml, read_toml_file, write_toml_file};
use genubar_core::{LEGACY_EXTENSION_UUID, MenuError};
use genubar_platform::fs_utils::write_text_file;
use std::path::Path;

pub struct SettingsRepo;

impl SettingsRepo {
    pub fn load(base_dir: &Path) -> Result<GlobalSettings, MenuError> {
        let mut settings: GlobalSettings = read_toml_file(&base_dir.join("settings.toml"))?;
        let legacy_suffix =
            format!("gnome-shell/extensions/{LEGACY_EXTENSION_UUID}/stylesheet.css");
        if settings
            .style
            .extension_css_path
            .to_string_lossy()
            .ends_with(&legacy_suffix)
        {
            settings.style.extension_css_path = StyleFileConfig::default().extension_css_path;
            let _ = Self::save(base_dir, &settings);
        }
        Ok(settings)
    }

    pub fn save(base_dir: &Path, settings: &GlobalSettings) -> Result<(), MenuError> {
        write_toml_file(&base_dir.join("settings.toml"), settings)
    }

    pub fn defaults() -> Result<GlobalSettings, MenuError> {
        parse_embedded_toml(DEFAULT_SETTINGS_TOML, "settings.toml")
    }

    pub fn reset_styles_to_defaults(
        base_dir: &Path,
        settings: &mut GlobalSettings,
    ) -> Result<(), MenuError> {
        let def_settings = Self::defaults()?;
        settings.style = def_settings.style;
        write_text_file(base_dir.join("styles/default.css"), DEFAULT_STYLE_CSS)?;
        write_text_file(base_dir.join("style.css"), DEFAULT_STYLE_CSS)?;
        Self::save(base_dir, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsRepo;
    use genubar_core::{EXTENSION_UUID, LEGACY_EXTENSION_UUID};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_migrates_legacy_extension_css_path() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("genubar_settings_migr_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.toml"),
            format!(
                "[style]\nextension_css_path = \"~/.local/share/gnome-shell/extensions/{LEGACY_EXTENSION_UUID}/stylesheet.css\"\n"
            ),
        )
        .unwrap();

        let loaded = SettingsRepo::load(&dir).unwrap();
        let expected_suffix = format!("gnome-shell/extensions/{EXTENSION_UUID}/stylesheet.css");
        assert!(loaded
            .style
            .extension_css_path
            .to_string_lossy()
            .ends_with(&expected_suffix));

        let saved_on_disk = std::fs::read_to_string(dir.join("settings.toml")).unwrap();
        assert!(saved_on_disk.contains(EXTENSION_UUID));
        assert!(!saved_on_disk.contains(LEGACY_EXTENSION_UUID));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
