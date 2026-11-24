use crate::defaults::ConfigBootstrapper;
use crate::models::{
    icons::{IconPackManifest, IconsConfig},
    menu::{AppMenuTemplateConfig, DesktopMenuConfig},
    profile::AppProfile,
    settings::{CssSyncMode, GlobalSettings},
};
use crate::repos::{IconPackRepo, ProfileRepo, SettingsRepo, TemplatesRepo};
use crate::services::{snapshot_service::SnapshotStore, styles_service::StylesManager};
use genubar_core::{ActiveWindowInfo, MenuError};
use genubar_platform::{
    CssMetrics, CssSyncModeKind, CssSyncOptions, CssSynchronizer, Gtk4EnvSyncOptions,
    Gtk4EnvSynchronizer, config_dir,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigManager {
    base_dir: PathBuf,
    pub settings: GlobalSettings,
    pub icons: IconsConfig,
    pub icon_packs: Vec<IconPackManifest>,
    pub desktop_menu: DesktopMenuConfig,
    pub app_menu: AppMenuTemplateConfig,
    pub profiles: Vec<AppProfile>,
    pub snapshots: SnapshotStore,
    pub styles: StylesManager,
}

impl ConfigManager {
    pub fn default_base_dir() -> PathBuf {
        config_dir()
    }

    pub fn load_or_bootstrap() -> Result<Self, MenuError> {
        Self::load_from_base_dir(Self::default_base_dir())
    }

    pub fn load_from_base_dir(base_dir: PathBuf) -> Result<Self, MenuError> {
        ConfigBootstrapper::bootstrap_if_missing(&base_dir)?;

        let settings = SettingsRepo::load(&base_dir)?;
        let icons = TemplatesRepo::load_icons(&base_dir)?;
        let icon_packs = IconPackRepo::load_all(&base_dir.join("icons"));
        let desktop_menu = TemplatesRepo::load_desktop_menu(&base_dir)?;
        let app_menu = TemplatesRepo::load_app_menu(&base_dir)?;
        let profiles = ProfileRepo::load_all(&base_dir.join("profiles"));
        let snapshots = SnapshotStore::new(base_dir.join("snapshots"));
        let styles = StylesManager::new(base_dir.join("styles"));

        Ok(Self {
            base_dir,
            settings,
            icons,
            icon_packs,
            desktop_menu,
            app_menu,
            profiles,
            snapshots,
            styles,
        })
    }

    pub fn sync_platform_env(&self) {
        self.sync_stylesheet();
        self.sync_gtk4_hook_environment();
    }

    pub fn sync_stylesheet(&self) {
        let style = &self.settings.style;
        let sync_mode = match style.sync_mode {
            CssSyncMode::SymlinkToExtension => CssSyncModeKind::SymlinkToExtension,
            CssSyncMode::CopyToExtension => CssSyncModeKind::CopyToExtension,
            CssSyncMode::DirectExtensionFile => CssSyncModeKind::DirectExtensionFile,
        };
        let opts = CssSyncOptions {
            sync_mode,
            active_style: &style.active_style,
            custom_css_path: &style.custom_css_path,
            extension_css_path: &style.extension_css_path,
            apply_metrics_to_css: style.apply_metrics_to_css,
            metrics: CssMetrics {
                panel_button_hpadding_px: style.panel_button_hpadding_px,
                popup_border_radius_px: style.popup_border_radius_px,
                max_column_height_px: style.max_column_height_px,
                min_column_width_px: style.min_column_width_px,
                item_vpadding_px: style.item_vpadding_px,
                item_hpadding_px: style.item_hpadding_px,
                item_border_radius_px: style.item_border_radius_px,
                font_size_px: style.font_size_px,
            },
        };
        CssSynchronizer::sync(&self.base_dir, &opts);
    }

    pub fn sync_gtk4_hook_environment(&self) {
        let hook_cfg = &self.settings.api_tiers.gtk4_hook;
        let opts = Gtk4EnvSyncOptions {
            hide_original_buttons: hook_cfg.hide_original_buttons,
            custom_hook_so_path: hook_cfg.custom_hook_so_path.as_deref(),
        };
        Gtk4EnvSynchronizer::sync(&self.base_dir, &opts);
    }

    pub fn find_matching_profile(&self, info: &ActiveWindowInfo) -> Option<&AppProfile> {
        self.profiles.iter().find(|p| p.matches_window(info))
    }

    pub fn find_matching_profiles(&self, info: &ActiveWindowInfo) -> Vec<&AppProfile> {
        let mut matched: Vec<&AppProfile> = self
            .profiles
            .iter()
            .filter(|p| p.matches_window(info))
            .collect();
        matched.sort_by_key(|p| p.priority);
        matched
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn save_settings(&self) -> Result<(), MenuError> {
        SettingsRepo::save(&self.base_dir, &self.settings)
    }

    pub fn save_icons(&self) -> Result<(), MenuError> {
        TemplatesRepo::save_icons(&self.base_dir, &self.icons)
    }

    pub fn save_app_menu(&self) -> Result<(), MenuError> {
        TemplatesRepo::save_app_menu(&self.base_dir, &self.app_menu)
    }

    pub fn save_desktop_menu(&self) -> Result<(), MenuError> {
        TemplatesRepo::save_desktop_menu(&self.base_dir, &self.desktop_menu)
    }

    pub fn save_profile(&mut self, profile: AppProfile) -> Result<PathBuf, MenuError> {
        ProfileRepo::save(&self.base_dir.join("profiles"), &mut self.profiles, profile)
    }

    pub fn delete_profile(&mut self, profile_id: &str) -> Result<(), MenuError> {
        ProfileRepo::delete(
            &self.base_dir.join("profiles"),
            &mut self.profiles,
            profile_id,
        )
    }

    pub fn save_icon_pack(&self, pack: &IconPackManifest) -> Result<(), MenuError> {
        IconPackRepo::save(pack)
    }

    pub fn reset_settings_to_defaults(&mut self) -> Result<(), MenuError> {
        self.settings = SettingsRepo::defaults()?;
        self.save_settings()?;
        self.sync_platform_env();
        Ok(())
    }

    pub fn reset_styles_to_defaults(&mut self) -> Result<(), MenuError> {
        SettingsRepo::reset_styles_to_defaults(&self.base_dir, &mut self.settings)?;
        self.sync_stylesheet();
        Ok(())
    }

    pub fn reset_icons_to_defaults(&mut self) -> Result<(), MenuError> {
        self.icons = TemplatesRepo::default_icons()?;
        self.save_icons()
    }

    pub fn reset_system_menus_to_defaults(&mut self) -> Result<(), MenuError> {
        let def_settings = SettingsRepo::defaults()?;
        self.settings.root_sorting = def_settings.root_sorting;
        self.save_settings()?;

        self.app_menu = TemplatesRepo::default_app_menu()?;
        self.save_app_menu()?;

        self.desktop_menu = TemplatesRepo::default_desktop_menu()?;
        self.save_desktop_menu()
    }

    pub fn reset_profiles_to_defaults(&mut self) -> Result<(), MenuError> {
        ConfigBootstrapper::overwrite_default_profiles(&self.base_dir)?;
        self.profiles = ProfileRepo::load_all(&self.base_dir.join("profiles"));
        Ok(())
    }

    pub fn reset_all_to_factory_defaults(&mut self) -> Result<(), MenuError> {
        ConfigBootstrapper::overwrite_all_defaults(&self.base_dir)?;
        *self = Self::load_from_base_dir(self.base_dir.clone())?;
        self.sync_platform_env();
        Ok(())
    }
}
