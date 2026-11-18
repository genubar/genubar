use crate::defaults::{DEFAULT_APP_MENU_TOML, DEFAULT_DESKTOP_TOML, DEFAULT_ICONS_TOML};
use crate::models::{
    icons::IconsConfig,
    menu::{AppMenuTemplateConfig, DesktopMenuConfig},
};
use crate::utils::toml_io::{parse_embedded_toml, read_toml_file, write_toml_file};
use genubar_core::MenuError;
use std::path::Path;

pub struct TemplatesRepo;

impl TemplatesRepo {
    pub fn load_icons(base_dir: &Path) -> Result<IconsConfig, MenuError> {
        read_toml_file(&base_dir.join("icons.toml"))
    }

    pub fn save_icons(base_dir: &Path, icons: &IconsConfig) -> Result<(), MenuError> {
        write_toml_file(&base_dir.join("icons.toml"), icons)
    }

    pub fn default_icons() -> Result<IconsConfig, MenuError> {
        parse_embedded_toml(DEFAULT_ICONS_TOML, "icons.toml")
    }

    pub fn load_desktop_menu(base_dir: &Path) -> Result<DesktopMenuConfig, MenuError> {
        read_toml_file(&base_dir.join("desktop_menu.toml"))
    }

    pub fn save_desktop_menu(
        base_dir: &Path,
        desktop_menu: &DesktopMenuConfig,
    ) -> Result<(), MenuError> {
        write_toml_file(&base_dir.join("desktop_menu.toml"), desktop_menu)
    }

    pub fn default_desktop_menu() -> Result<DesktopMenuConfig, MenuError> {
        parse_embedded_toml(DEFAULT_DESKTOP_TOML, "desktop_menu.toml")
    }

    pub fn load_app_menu(base_dir: &Path) -> Result<AppMenuTemplateConfig, MenuError> {
        read_toml_file(&base_dir.join("app_menu.toml"))
    }

    pub fn save_app_menu(
        base_dir: &Path,
        app_menu: &AppMenuTemplateConfig,
    ) -> Result<(), MenuError> {
        write_toml_file(&base_dir.join("app_menu.toml"), app_menu)
    }

    pub fn default_app_menu() -> Result<AppMenuTemplateConfig, MenuError> {
        parse_embedded_toml(DEFAULT_APP_MENU_TOML, "app_menu.toml")
    }
}
