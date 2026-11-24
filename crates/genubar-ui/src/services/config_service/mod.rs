use crate::error::UiError;
use genubar_config::{
    AppMatcher, AppMenuTemplateConfig, AppProfile, ConfigManager, DesktopMenuConfig,
    IconPackManifest, MatchField, MenuNodeDef,
};
use genubar_patcher::PatchManager;
use genubar_pipeline::PythonRunner;
use genubar_platform::fs_utils::read_text_file;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ConfigService;

fn shift_position(idx: usize, len: usize, delta: isize) -> Option<usize> {
    let target = idx as isize + delta;
    if target < 0 || target as usize >= len {
        None
    } else {
        Some(target as usize)
    }
}

impl ConfigService {
    pub fn reload_config() -> Result<ConfigManager, UiError> {
        ConfigManager::load_or_bootstrap().map_err(UiError::Domain)
    }

    pub fn reload_config_and_patcher() -> Result<(ConfigManager, PatchManager), UiError> {
        let cfg = ConfigManager::load_or_bootstrap()?;
        let patcher = PatchManager::load_or_init()?;
        Ok((cfg, patcher))
    }

    pub fn move_profile_by_id(
        cfg: &mut ConfigManager,
        profile_id: &str,
        delta: isize,
    ) -> Result<bool, UiError> {
        let Some(from_idx) = cfg.profiles.iter().position(|p| p.id == profile_id) else {
            return Ok(false);
        };
        let Some(to_idx) = shift_position(from_idx, cfg.profiles.len(), delta) else {
            return Ok(false);
        };
        cfg.profiles.swap(from_idx, to_idx);
        let reordered: Vec<AppProfile> = cfg
            .profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut cloned = p.clone();
                cloned.priority = (i as i32) * 10;
                cloned
            })
            .collect();

        for p in reordered {
            cfg.save_profile(p)?;
        }
        cfg.profiles.sort_by_key(|p| p.priority);
        Ok(true)
    }

    pub fn set_profile_enabled(
        cfg: &mut ConfigManager,
        profile_id: &str,
        enabled: bool,
    ) -> Result<bool, UiError> {
        let Some(mut updated) = cfg.profiles.iter().find(|p| p.id == profile_id).cloned() else {
            return Ok(false);
        };
        updated.enabled = enabled;
        cfg.save_profile(updated)?;
        cfg.profiles.sort_by_key(|p| p.priority);
        Ok(true)
    }

    pub fn disable_all_profiles(cfg: &mut ConfigManager) -> Result<(), UiError> {
        let updated: Vec<AppProfile> = cfg
            .profiles
            .iter()
            .map(|p| {
                let mut c = p.clone();
                c.enabled = false;
                c
            })
            .collect();
        for p in updated {
            cfg.save_profile(p)?;
        }
        cfg.profiles.sort_by_key(|p| p.priority);
        Ok(())
    }

    pub fn create_default_profile(cfg: &mut ConfigManager) -> Result<AppProfile, UiError> {
        let next_prio = (cfg.profiles.len() as i32) * 10;
        let suffix = cfg.profiles.len() + 1;
        let template = AppProfile {
            id: format!("custom_profile_{suffix}"),
            name: format!("Custom Profile #{suffix}"),
            version: String::from("1.0.0"),
            author: String::from("user"),
            description: String::from("Custom application or global plugin menu"),
            min_app_version: None,
            enabled: true,
            priority: next_prio,
            matchers: vec![AppMatcher {
                field: MatchField::Wildcard,
                contains: String::from("*"),
            }],
            sync_new_items: true,
            include_app_system_column: true,
            fallback_only: false,
            disable_root_sort: false,
            allowed_tiers: None,
            atspi_override: None,
            volatile_submenus: vec![],
            columns: vec![MenuNodeDef::InheritOthers],
            python_script: None,
            file_path: PathBuf::new(),
        };
        cfg.save_profile(template.clone())?;
        cfg.profiles.sort_by_key(|p| p.priority);
        Ok(template)
    }

    pub fn import_profile_from_path(
        cfg: &mut ConfigManager,
        path: &Path,
    ) -> Result<String, UiError> {
        let raw = read_text_file(path)?;
        let prof: AppProfile = toml::from_str(&raw)?;
        let name = prof.name.clone();
        cfg.save_profile(prof)?;
        cfg.profiles.sort_by_key(|p| p.priority);
        Ok(name)
    }

    pub fn move_icon_pack_by_id(
        cfg: &mut ConfigManager,
        pack_id: &str,
        delta: isize,
    ) -> Result<bool, UiError> {
        let Some(from_idx) = cfg.icon_packs.iter().position(|p| p.id == pack_id) else {
            return Ok(false);
        };
        let Some(to_idx) = shift_position(from_idx, cfg.icon_packs.len(), delta) else {
            return Ok(false);
        };
        cfg.icon_packs.swap(from_idx, to_idx);
        let updated: Vec<IconPackManifest> = cfg
            .icon_packs
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut cloned = p.clone();
                cloned.priority = (i as i32) * 10;
                cloned
            })
            .collect();

        for p in &updated {
            cfg.save_icon_pack(p)?;
        }
        cfg.icon_packs = updated;
        Ok(true)
    }

    pub fn set_icon_pack_enabled(
        cfg: &mut ConfigManager,
        pack_id: &str,
        enabled: bool,
    ) -> Result<bool, UiError> {
        let Some(pack) = cfg.icon_packs.iter_mut().find(|p| p.id == pack_id) else {
            return Ok(false);
        };
        pack.enabled = enabled;
        let clone = pack.clone();
        cfg.save_icon_pack(&clone)?;
        Ok(true)
    }

    pub fn disable_all_icon_packs(cfg: &mut ConfigManager) -> Result<(), UiError> {
        for i in 0..cfg.icon_packs.len() {
            cfg.icon_packs[i].enabled = false;
            let pack = cfg.icon_packs[i].clone();
            cfg.save_icon_pack(&pack)?;
        }
        Ok(())
    }

    pub fn add_exclusion_pattern(cfg: &mut ConfigManager, raw_pattern: &str) -> bool {
        let pat = raw_pattern.trim().to_string();
        if pat.is_empty()
            || cfg
                .settings
                .root_sorting
                .exclude_apps
                .iter()
                .any(|p| p == &pat)
        {
            return false;
        }
        cfg.settings.root_sorting.exclude_apps.push(pat);
        Self::sort_exclusion_patterns(cfg);
        true
    }

    pub fn update_exclusion_pattern(
        cfg: &mut ConfigManager,
        old_pattern: &str,
        new_pattern: &str,
    ) -> bool {
        let trimmed = new_pattern.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(slot) = cfg
            .settings
            .root_sorting
            .exclude_apps
            .iter_mut()
            .find(|p| p.as_str() == old_pattern)
        else {
            return false;
        };
        *slot = trimmed.to_string();
        true
    }

    pub fn remove_exclusion_pattern(cfg: &mut ConfigManager, pattern: &str) -> bool {
        let prev_len = cfg.settings.root_sorting.exclude_apps.len();
        cfg.settings
            .root_sorting
            .exclude_apps
            .retain(|p| p != pattern);
        cfg.settings.root_sorting.exclude_apps.len() < prev_len
    }

    pub fn sort_exclusion_patterns(cfg: &mut ConfigManager) {
        cfg.settings
            .root_sorting
            .exclude_apps
            .sort_by_key(|a| a.to_lowercase());
    }

    pub fn update_registry_url(
        cfg: &mut ConfigManager,
        old_url: &str,
        new_url: &str,
    ) -> Result<bool, UiError> {
        let trimmed = new_url.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }
        let Some(slot) = cfg
            .settings
            .general
            .registry_urls
            .iter_mut()
            .find(|u| u.as_str() == old_url)
        else {
            return Ok(false);
        };
        *slot = trimmed.to_string();
        cfg.save_settings()?;
        Ok(true)
    }

    pub fn remove_registry_url(cfg: &mut ConfigManager, url: &str) -> Result<bool, UiError> {
        let prev_len = cfg.settings.general.registry_urls.len();
        cfg.settings.general.registry_urls.retain(|u| u != url);
        if cfg.settings.general.registry_urls.len() < prev_len {
            cfg.save_settings()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn read_active_style_css(cfg: &ConfigManager) -> Result<String, UiError> {
        let active_file = cfg
            .styles
            .styles_dir()
            .join(&cfg.settings.style.active_style);
        if !active_file.exists() {
            return Ok(String::new());
        }
        read_text_file(&active_file).map_err(UiError::Domain)
    }

    pub fn serialize_app_menu(cfg: &ConfigManager) -> String {
        toml::to_string_pretty(&cfg.app_menu).unwrap_or_default()
    }

    pub fn validate_and_save_app_menu(
        cfg: &mut ConfigManager,
        raw_toml: &str,
    ) -> Result<(), UiError> {
        let parsed: AppMenuTemplateConfig = toml::from_str(raw_toml)?;
        cfg.app_menu = parsed;
        cfg.save_app_menu().map_err(UiError::Domain)
    }

    pub fn serialize_desktop_menu(cfg: &ConfigManager) -> String {
        toml::to_string_pretty(&cfg.desktop_menu).unwrap_or_default()
    }

    pub fn validate_and_save_desktop_menu(
        cfg: &mut ConfigManager,
        raw_toml: &str,
    ) -> Result<(), UiError> {
        let parsed: DesktopMenuConfig = toml::from_str(raw_toml)?;
        cfg.desktop_menu = parsed;
        cfg.save_desktop_menu().map_err(UiError::Domain)
    }

    pub fn list_snapshot_files(cfg: &ConfigManager) -> Vec<(String, PathBuf, bool)> {
        let mut out = Vec::new();
        let snaps_dir = cfg.base_dir().join("snapshots");
        let profile_ids: HashSet<String> =
            cfg.profiles.iter().map(|p| p.id.to_lowercase()).collect();

        if let Ok(entries) = fs::read_dir(snaps_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("json") {
                    let fname = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("snapshot.json")
                        .to_string();
                    let stem = fname.trim_end_matches(".json").to_lowercase();
                    let is_used = profile_ids.contains(&stem);
                    out.push((fname, p, is_used));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn create_profile_from_snapshot_and_reload(
        cfg: &ConfigManager,
        snap_path: &Path,
    ) -> Result<(PathBuf, ConfigManager), UiError> {
        let prof_dir = cfg.base_dir().join("profiles");
        let out_path = cfg
            .snapshots
            .create_profile_from_snapshot(snap_path, &prof_dir, true)?;
        let fresh_cfg = Self::reload_config()?;
        Ok((out_path, fresh_cfg))
    }

    pub fn delete_snapshot(path: &Path) -> bool {
        fs::remove_file(path).is_ok()
    }

    pub fn delete_unused_snapshots(cfg: &ConfigManager) -> usize {
        let mut deleted = 0;
        for (_fname, path, is_used) in Self::list_snapshot_files(cfg) {
            if !is_used && fs::remove_file(path).is_ok() {
                deleted += 1;
            }
        }
        deleted
    }

    pub fn list_scripts_by_kind(cfg: &ConfigManager, allowed_kinds: &[&str]) -> Vec<String> {
        let mut scripts = Vec::new();
        let scripts_dir = cfg.base_dir().join("scripts");
        if let Ok(entries) = fs::read_dir(scripts_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("py")
                    && let Some(meta) = PythonRunner::parse_and_validate_header(&p)
                    && allowed_kinds
                        .iter()
                        .any(|kind| meta.kind.eq_ignore_ascii_case(kind))
                    && let Some(fname) = p.file_name().and_then(|n| n.to_str())
                {
                    scripts.push(format!("scripts/{fname}"));
                }
            }
        }
        scripts.sort();
        scripts
    }

    pub fn list_valid_scripts(cfg: &ConfigManager) -> Vec<String> {
        Self::list_scripts_by_kind(cfg, &["menu_transformer"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_config::IconPackManifest;

    fn temp_cfg(name: &str) -> ConfigManager {
        let base = std::env::temp_dir().join(format!("genubar_ui_cfg_test_{name}"));
        let _ = fs::remove_dir_all(&base);
        ConfigManager::load_from_base_dir(base).unwrap()
    }

    #[test]
    fn stable_id_and_pattern_mutations_preserve_identity() {
        let mut cfg = temp_cfg("stable_ids");
        assert!(ConfigService::add_exclusion_pattern(
            &mut cfg,
            "org.gnome.Calculator"
        ));
        assert!(ConfigService::add_exclusion_pattern(&mut cfg, "*spotify*"));
        assert!(!ConfigService::add_exclusion_pattern(&mut cfg, "*spotify*"));
        assert!(ConfigService::update_exclusion_pattern(
            &mut cfg,
            "*spotify*",
            "*Spotify*"
        ));
        assert!(ConfigService::remove_exclusion_pattern(
            &mut cfg,
            "org.gnome.Calculator"
        ));
        assert_eq!(
            cfg.settings.root_sorting.exclude_apps,
            vec![String::from("*Spotify*")]
        );

        let icons_base = cfg.base_dir().join("icons");
        let pack_a = IconPackManifest {
            id: String::from("pack_a"),
            name: String::from("Pack A"),
            priority: 0,
            enabled: true,
            pack_dir: icons_base.join("pack_a"),
            ..Default::default()
        };
        let pack_b = IconPackManifest {
            id: String::from("pack_b"),
            name: String::from("Pack B"),
            priority: 10,
            enabled: false,
            pack_dir: icons_base.join("pack_b"),
            ..Default::default()
        };
        cfg.save_icon_pack(&pack_a).unwrap();
        cfg.save_icon_pack(&pack_b).unwrap();
        cfg.icon_packs = vec![pack_a, pack_b];

        assert!(ConfigService::move_icon_pack_by_id(&mut cfg, "pack_b", -1).unwrap());
        assert_eq!(cfg.icon_packs[0].id, "pack_b");
        assert!(ConfigService::set_icon_pack_enabled(&mut cfg, "pack_a", false).unwrap());
        assert!(!cfg.icon_packs[1].enabled);

        let _ = fs::remove_dir_all(cfg.base_dir());
    }

    #[test]
    fn list_scripts_filters_menu_transformers_from_patch_scripts() {
        let cfg = temp_cfg("script_kinds");
        let custom_default_kind = cfg.base_dir().join("scripts/custom_default.py");
        fs::write(
            &custom_default_kind,
            "# ==GenubarScript==\n# @id custom_default\n# @name Custom\n# ==/GenubarScript==\n",
        )
        .unwrap();

        let menu_scripts = ConfigService::list_valid_scripts(&cfg);
        assert!(menu_scripts.contains(&String::from("scripts/example_menu_transformer.py")));
        assert!(menu_scripts.contains(&String::from("scripts/custom_default.py")));
        assert!(!menu_scripts.contains(&String::from("scripts/example_patch_script.py")));

        let patch_scripts = ConfigService::list_scripts_by_kind(&cfg, &["patch_script"]);
        assert_eq!(
            patch_scripts,
            vec![String::from("scripts/example_patch_script.py")]
        );

        let _ = fs::remove_dir_all(cfg.base_dir());
    }
}
