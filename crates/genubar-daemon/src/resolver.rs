use crate::fallback::FallbackMenuBuilder;
use crate::state::{DaemonCaches, WindowRegistry};
use crate::tier_acquirer::TierAcquirer;
use genubar_config::ConfigManager;
use genubar_core::{ActiveWindowInfo, MenuItemType, MenuNode, MenuSourceTier, MenuTarget};
use genubar_dbus::UnifiedMenuService;
use genubar_pipeline::{MenuPipeline, PipelineRuntimeConfig};
use std::sync::Arc;

pub struct WindowMenuResolver;

impl WindowMenuResolver {
    pub fn runtime_config(cfg: &ConfigManager) -> PipelineRuntimeConfig {
        PipelineRuntimeConfig {
            script_timeout_ms: cfg.settings.general.script_timeout_ms,
            show_menu_icons: cfg.settings.general.show_menu_icons,
            auto_map_standard_icons: cfg.settings.general.auto_map_standard_icons,
            enable_menu_search: cfg.settings.general.enable_menu_search,
        }
    }

    /// Fast-path L1/RAM lookup used to render the top panel in <1ms before live D-Bus polling.
    pub async fn resolve_fast_cached_menu(
        caches: &DaemonCaches,
        cfg: &Arc<ConfigManager>,
        info: &ActiveWindowInfo,
    ) -> Option<(Option<MenuTarget>, MenuNode, MenuSourceTier)> {
        if info.pid == 0 && info.xid == 0 {
            return None;
        }

        let cached_entry = if cfg.settings.api_tiers.gtk4_hook.enabled {
            let hooked_target = caches
                .windows
                .resolve_gtk4(info.pid, &info.gtk_bus_name)
                .await;
            if hooked_target.is_some() || WindowRegistry::is_process_gtk4_hooked(info.pid) {
                caches
                    .l1
                    .get_menu(info)
                    .await
                    .map(|l1_menu| (hooked_target, l1_menu, MenuSourceTier::Gtk4Hook, true))
            } else {
                None
            }
        } else {
            None
        };

        let (target_opt, raw_menu, tier, post_process) = match cached_entry {
            Some(hit) => hit,
            None => {
                let target = caches.windows.resolve_canonical(info.xid, info.pid).await?;
                let raw_menu =
                    if let Some(ram_menu) = caches.canonical.get_cached_menu(info.pid).await {
                        ram_menu
                    } else {
                        caches.l1.get_menu(info).await?
                    };
                (Some(target), raw_menu, MenuSourceTier::CanonicalDbus, false)
            }
        };

        let cfg_c = Arc::clone(cfg);
        let info_c = info.clone();
        let final_root = tokio::task::spawn_blocking(move || {
            let menu = if post_process {
                Self::apply_tier_post_processing(&cfg_c, &info_c, tier, raw_menu)
            } else {
                raw_menu
            };
            Self::finalize_menu_pipeline(&cfg_c, &info_c, menu, tier, false)
        })
        .await
        .ok()?;

        Some((target_opt, final_root, tier))
    }

    pub async fn resolve_and_fetch(
        service: &UnifiedMenuService,
        caches: &DaemonCaches,
        cfg: &Arc<ConfigManager>,
        info: &ActiveWindowInfo,
    ) -> (Option<MenuTarget>, MenuNode, MenuSourceTier) {
        if info.pid == 0 && info.xid == 0 {
            let cfg_c = Arc::clone(cfg);
            let info_c = info.clone();
            let desktop_root =
                tokio::task::spawn_blocking(move || Self::resolve_desktop_menu(&cfg_c, &info_c))
                    .await
                    .unwrap_or_else(|_| FallbackMenuBuilder::build_desktop_menu(cfg, info));
            return (None, desktop_root, MenuSourceTier::DesktopIdle);
        }

        let matched_profiles = cfg.find_matching_profiles(info);
        let allowed_tiers: Option<Vec<MenuSourceTier>> = matched_profiles
            .iter()
            .find_map(|p| p.allowed_tiers.clone());
        let has_profile_fallback = matched_profiles
            .iter()
            .any(|p| !p.columns.is_empty() || p.python_script.is_some());

        let mut raw_acquired =
            TierAcquirer::acquire_raw_menu(service, caches, cfg, info, allowed_tiers.as_deref())
                .await;

        if raw_acquired.is_none() && allowed_tiers.is_none() {
            let fast_recheck = [MenuSourceTier::Gtk4Hook, MenuSourceTier::CanonicalDbus];
            raw_acquired =
                TierAcquirer::acquire_raw_menu(service, caches, cfg, info, Some(&fast_recheck))
                    .await;
        }

        let cfg_c = Arc::clone(cfg);
        let info_c = info.clone();

        tokio::task::spawn_blocking(move || {
            let (target_opt, live_menu, tier) = match raw_acquired {
                Some((target, raw_menu, tier)) => {
                    let processed =
                        Self::apply_tier_post_processing(&cfg_c, &info_c, tier, raw_menu);
                    (Some(target), processed, tier)
                }
                None => {
                    let empty_live = MenuNode {
                        id: 0,
                        label: String::from("Root"),
                        enabled: true,
                        visible: true,
                        item_type: MenuItemType::Submenu,
                        shortcut: None,
                        action: None,
                        icon: None,
                        origin_path: None,
                        children: vec![],
                    };
                    (None, empty_live, MenuSourceTier::AppFallback)
                }
            };

            let is_fallback_state = target_opt.is_none();
            let final_tier = if is_fallback_state && has_profile_fallback {
                MenuSourceTier::AppProfile
            } else {
                tier
            };

            let final_root = Self::finalize_menu_pipeline(
                &cfg_c,
                &info_c,
                live_menu,
                final_tier,
                is_fallback_state,
            );
            (target_opt, final_root, final_tier)
        })
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(error = %err, "Pipeline blocking task panicked");
            (
                None,
                FallbackMenuBuilder::build_desktop_menu(cfg, info),
                MenuSourceTier::AppFallback,
            )
        })
    }

    pub fn resolve_desktop_menu(cfg: &ConfigManager, info: &ActiveWindowInfo) -> MenuNode {
        let python_bin = &cfg.settings.general.python_binary;
        let base_dir = cfg.base_dir();
        let sys_title = cfg.desktop_menu.resolved_system_title();
        let runtime = Self::runtime_config(cfg);
        let mut desktop_root = FallbackMenuBuilder::build_desktop_menu(cfg, info);

        for profile in cfg.find_matching_profiles(info) {
            desktop_root = MenuPipeline::apply_profile_with_config(
                desktop_root,
                profile,
                info,
                MenuSourceTier::DesktopIdle,
                python_bin,
                base_dir,
                &sys_title,
                runtime,
            );
        }

        MenuPipeline::apply_menu_icons_with_config(
            &mut desktop_root,
            info,
            &cfg.icons,
            &cfg.icon_packs,
            runtime,
        );
        MenuPipeline::apply_runtime_filters(&mut desktop_root, runtime, true);
        desktop_root
    }

    pub fn apply_tier_post_processing(
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        mut live_menu: MenuNode,
    ) -> MenuNode {
        let python_bin = &cfg.settings.general.python_binary;
        let base_dir = cfg.base_dir();
        let timeout_ms = cfg.settings.general.script_timeout_ms;

        match tier {
            MenuSourceTier::GtkNative => {
                if let Some(ref script) = cfg.settings.api_tiers.gtk_native.python_script
                    && let Some(out) = MenuPipeline::run_python_script_with_timeout(
                        python_bin, base_dir, script, info, tier, &live_menu, timeout_ms,
                    )
                {
                    live_menu = out;
                }
            }
            MenuSourceTier::Gtk4Hook => {
                let hook_cfg = &cfg.settings.api_tiers.gtk4_hook;
                live_menu = MenuPipeline::apply_gtk4_layout_with_timeout(
                    live_menu,
                    hook_cfg.layout_algorithm,
                    hook_cfg.expose_unknown_custom_widgets,
                    hook_cfg.python_script.as_deref(),
                    python_bin,
                    base_dir,
                    info,
                    timeout_ms,
                );
            }
            MenuSourceTier::CanonicalDbus => {
                if let Some(ref script) = cfg.settings.api_tiers.canonical_dbus.python_script
                    && let Some(out) = MenuPipeline::run_python_script_with_timeout(
                        python_bin, base_dir, script, info, tier, &live_menu, timeout_ms,
                    )
                {
                    live_menu = out;
                }
            }
            MenuSourceTier::Atspi => {
                if let Some(ref script) = cfg.settings.api_tiers.atspi.python_script
                    && let Some(out) = MenuPipeline::run_python_script_with_timeout(
                        python_bin, base_dir, script, info, tier, &live_menu, timeout_ms,
                    )
                {
                    live_menu = out;
                }
            }
            _ => {}
        }

        live_menu
    }

    pub fn finalize_menu_pipeline(
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
        mut app_columns_root: MenuNode,
        tier: MenuSourceTier,
        is_fallback_state: bool,
    ) -> MenuNode {
        let app_name = FallbackMenuBuilder::clean_app_name(info);
        let python_bin = &cfg.settings.general.python_binary;
        let base_dir = cfg.base_dir();
        let runtime = Self::runtime_config(cfg);
        let matched_profiles = cfg.find_matching_profiles(info);

        let mut wrap_in_app_template = true;
        for profile in &matched_profiles {
            if profile.fallback_only && !is_fallback_state {
                continue;
            }
            if !profile.columns.is_empty() || profile.python_script.is_some() {
                if !profile.include_app_system_column {
                    wrap_in_app_template = false;
                }
                app_columns_root = MenuPipeline::apply_profile_with_config(
                    app_columns_root,
                    profile,
                    info,
                    tier,
                    python_bin,
                    base_dir,
                    &app_name,
                    runtime,
                );
            }
        }

        let mut final_root = if wrap_in_app_template {
            MenuPipeline::apply_app_menu_template_with_config(
                app_columns_root,
                &cfg.app_menu,
                info,
                tier,
                python_bin,
                base_dir,
                &app_name,
                runtime,
            )
        } else {
            app_columns_root
        };

        let skip_sort = matched_profiles.iter().any(|p| p.disable_root_sort);
        if !skip_sort {
            final_root = MenuPipeline::apply_root_sorting_with_timeout(
                final_root,
                &cfg.settings.root_sorting,
                info,
                &app_name,
                python_bin,
                base_dir,
                runtime.script_timeout_ms,
            );
        }

        if cfg.settings.general.enable_more_overflow {
            MenuPipeline::apply_more_overflow(
                &mut final_root,
                cfg.settings.general.max_top_columns,
                &cfg.settings.general.more_column_label,
            );
        }

        MenuPipeline::apply_menu_icons_with_config(
            &mut final_root,
            info,
            &cfg.icons,
            &cfg.icon_packs,
            runtime,
        );
        let strip_refresh = tier == MenuSourceTier::Atspi
            && !cfg.settings.api_tiers.atspi.append_refresh_cache_button;
        MenuPipeline::apply_runtime_filters(&mut final_root, runtime, strip_refresh);
        final_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_config::ConfigManager;
    use std::fs;

    #[test]
    fn runtime_filters_honor_enable_menu_search_and_atspi_refresh_button() {
        let tmp_base = std::env::temp_dir().join("genubar_test_resolver_flags");
        let _ = fs::remove_dir_all(&tmp_base);
        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        cfg.settings.general.enable_menu_search = false;
        cfg.settings.api_tiers.atspi.append_refresh_cache_button = false;

        let info = ActiveWindowInfo {
            pid: 100,
            xid: 10,
            app_id: String::from("org.example.App"),
            title: String::from("Doc - App"),
            ..Default::default()
        };
        let empty = MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: Vec::new(),
        };

        let result_off = WindowMenuResolver::finalize_menu_pipeline(
            &cfg,
            &info,
            empty.clone(),
            MenuSourceTier::Atspi,
            false,
        );
        let json_off = serde_json::to_string(&result_off).unwrap();
        assert!(!json_off.contains("sys:refresh_menu"));
        assert!(!json_off.contains("SearchBar"));

        cfg.settings.general.enable_menu_search = true;
        cfg.settings.api_tiers.atspi.append_refresh_cache_button = true;
        let result_on = WindowMenuResolver::finalize_menu_pipeline(
            &cfg,
            &info,
            empty,
            MenuSourceTier::Atspi,
            false,
        );
        let json_on = serde_json::to_string(&result_on).unwrap();
        assert!(json_on.contains("sys:refresh_menu"));
        assert!(json_on.contains("SearchBar"));

        let _ = fs::remove_dir_all(&tmp_base);
    }

    #[test]
    fn desktop_menu_resolves_system_title_and_columns_offline() {
        let tmp_base = std::env::temp_dir().join("genubar_test_resolver_desktop");
        let _ = fs::remove_dir_all(&tmp_base);
        let cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let desktop_root =
            WindowMenuResolver::resolve_desktop_menu(&cfg, &ActiveWindowInfo::default());
        assert!(!desktop_root.children.is_empty());
        assert_eq!(
            desktop_root.children[0].clean_label(),
            cfg.desktop_menu.resolved_system_title()
        );
        let _ = fs::remove_dir_all(&tmp_base);
    }
}
