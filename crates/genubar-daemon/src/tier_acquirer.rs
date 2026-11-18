use crate::state::{DaemonCaches, WindowRegistry};
use genubar_config::ConfigManager;
use genubar_core::{ActiveWindowInfo, MenuNode, MenuProvider, MenuSourceTier, MenuTarget};
use genubar_dbus::{CanonicalFetchOptions, UnifiedMenuService};
use std::time::Duration;

pub struct TierAcquirer;

impl TierAcquirer {
    pub fn canonical_options(cfg: &ConfigManager) -> CanonicalFetchOptions {
        let can = &cfg.settings.api_tiers.canonical_dbus;
        CanonicalFetchOptions {
            preload_passes: can.preload_passes,
            swing_open_delay_ms: can.swing_open_delay_ms,
        }
    }

    pub async fn acquire_raw_menu(
        service: &UnifiedMenuService,
        caches: &DaemonCaches,
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
        allowed_tiers: Option<&[MenuSourceTier]>,
    ) -> Option<(MenuTarget, MenuNode, MenuSourceTier)> {
        let is_allowed = |tier: MenuSourceTier| -> bool {
            allowed_tiers.map(|list| list.contains(&tier)).unwrap_or(true)
        };

        let is_gtk4_hooked = info.pid != 0 && WindowRegistry::is_process_gtk4_hooked(info.pid);

        if is_allowed(MenuSourceTier::Gtk4Hook)
            && let Some(res) = Self::try_gtk4_hook(service, cfg, caches, info).await {
                return Some(res);
            }

        if is_allowed(MenuSourceTier::GtkNative)
            && let Some(res) = Self::try_gtk_native(service, cfg, info).await {
                return Some(res);
            }

        if is_allowed(MenuSourceTier::CanonicalDbus)
            && let Some(res) = Self::try_canonical_dbus(service, caches, cfg, info).await {
                return Some(res);
            }

        if is_allowed(MenuSourceTier::Atspi) && !is_gtk4_hooked
            && let Some(res) = Self::try_atspi(service, cfg, info).await {
                return Some(res);
            }

        None
    }

    async fn try_gtk_native(
        service: &UnifiedMenuService,
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
    ) -> Option<(MenuTarget, MenuNode, MenuSourceTier)> {
        if !cfg.settings.api_tiers.gtk_native.enabled
            || info.gtk_bus_name.is_empty()
            || (info.gtk_menubar_path.is_empty() && info.gtk_app_menu_path.is_empty())
        {
            return None;
        }

        let app_menu_path = if cfg.settings.api_tiers.gtk_native.include_app_menu_column {
            Some(info.gtk_app_menu_path.clone()).filter(|s| !s.is_empty())
        } else {
            None
        };

        let target = MenuTarget::Gtk {
            bus_name: info.gtk_bus_name.clone(),
            menubar_path: Some(info.gtk_menubar_path.clone()).filter(|s| !s.is_empty()),
            app_menu_path,
            app_path: Some(info.gtk_app_path.clone()).filter(|s| !s.is_empty()),
            win_path: Some(info.gtk_win_path.clone()).filter(|s| !s.is_empty()),
        };

        let menu = service.fetch_menu(&target).await.ok()?;
        Some((target, menu, MenuSourceTier::GtkNative))
    }

    async fn try_gtk4_hook(
        service: &UnifiedMenuService,
        cfg: &ConfigManager,
        caches: &DaemonCaches,
        info: &ActiveWindowInfo,
    ) -> Option<(MenuTarget, MenuNode, MenuSourceTier)> {
        if !cfg.settings.api_tiers.gtk4_hook.enabled || info.pid == 0 {
            return None;
        }

        let mut target_opt = caches.windows.resolve_gtk4(info.pid, &info.gtk_bus_name).await;
        if target_opt.is_none() && WindowRegistry::is_process_gtk4_hooked(info.pid) {
            for _ in 0..7 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                target_opt = caches.windows.resolve_gtk4(info.pid, &info.gtk_bus_name).await;
                if target_opt.is_some() {
                    break;
                }
            }
        }

        let target = match target_opt {
            Some(t) => t,
            None => {
                if info.gtk_bus_name.is_empty() {
                    return None;
                }
                let candidate = MenuTarget::Gtk {
                    bus_name: info.gtk_bus_name.clone(),
                    menubar_path: Some(String::from(genubar_protocol::GTK4_MENU_PATH)),
                    app_menu_path: None,
                    app_path: Some(String::from(genubar_protocol::GTK4_APP_PATH)),
                    win_path: Some(String::from(genubar_protocol::GTK4_WIN_PATH)),
                };
                if let Ok(Ok(menu)) = tokio::time::timeout(
                    Duration::from_millis(400),
                    service.fetch_menu(&candidate),
                )
                .await
                {
                    caches.windows.insert_gtk4(info.pid, candidate.clone()).await;
                    caches.l1.store_menu(info, &menu).await;
                    return Some((candidate, menu, MenuSourceTier::Gtk4Hook));
                }
                return None;
            }
        };

        if let Ok(Ok(menu)) =
            tokio::time::timeout(Duration::from_millis(1200), service.fetch_menu(&target)).await
        {
            caches.l1.store_menu(info, &menu).await;
            return Some((target, menu, MenuSourceTier::Gtk4Hook));
        }

        if let Some(l1_menu) = caches.l1.get_menu(info).await {
            return Some((target, l1_menu, MenuSourceTier::Gtk4Hook));
        }

        None
    }

    async fn try_canonical_dbus(
        service: &UnifiedMenuService,
        caches: &DaemonCaches,
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
    ) -> Option<(MenuTarget, MenuNode, MenuSourceTier)> {
        if !cfg.settings.api_tiers.canonical_dbus.enabled {
            return None;
        }

        let target = caches.windows.resolve_canonical(info.xid, info.pid).await?;
        let opts = Self::canonical_options(cfg);
        let fetch_res = tokio::time::timeout(
            Duration::from_millis(1100),
            service.canonical().fetch_menu_with_options(&target, opts),
        )
        .await;

        match fetch_res {
            Ok(Ok(mut menu)) => {
                if info.pid != 0 && cfg.settings.api_tiers.canonical_dbus.cache_last_valid_menu {
                    caches.canonical.stitch_submenus(info.pid, &mut menu).await;
                    caches.canonical.cache_menu(info.pid, menu.clone()).await;
                }
                caches.l1.store_menu(info, &menu).await;
                return Some((target, menu, MenuSourceTier::CanonicalDbus));
            }
            Ok(Err(genubar_core::MenuError::Transport(err))) => {
                tracing::debug!(
                    pid = info.pid,
                    error = %err,
                    "Canonical target vanished on D-Bus; invalidating stale target"
                );
                caches.windows.invalidate_canonical_pid(info.pid).await;
                return None;
            }
            Ok(Err(_)) | Err(_) => {}
        }

        if cfg.settings.api_tiers.canonical_dbus.cache_last_valid_menu {
            if let Some(cached_menu) = caches.canonical.get_cached_menu(info.pid).await {
                service
                    .canonical()
                    .seed_target_paths_from_menu(&target, &cached_menu)
                    .await;
                return Some((target, cached_menu, MenuSourceTier::CanonicalDbus));
            }
            if let Some(l1_menu) = caches.l1.get_menu(info).await {
                service
                    .canonical()
                    .seed_target_paths_from_menu(&target, &l1_menu)
                    .await;
                return Some((target, l1_menu, MenuSourceTier::CanonicalDbus));
            }
        }

        None
    }

    async fn try_atspi(
        service: &UnifiedMenuService,
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
    ) -> Option<(MenuTarget, MenuNode, MenuSourceTier)> {
        let atspi_cfg = &cfg.settings.api_tiers.atspi;
        if !atspi_cfg.enabled {
            return None;
        }

        service
            .atspi()
            .set_disk_cache_enabled(atspi_cfg.enable_disk_cache);

        let matched_profiles = cfg.find_matching_profiles(info);
        let volatiles = matched_profiles
            .iter()
            .find(|p| !p.volatile_submenus.is_empty())
            .map(|p| p.volatile_submenus.as_slice())
            .unwrap_or(&atspi_cfg.default_volatile_submenus);

        if !volatiles.is_empty() {
            service
                .atspi()
                .clear_cache_for_pid(info.pid, Some(volatiles))
                .await;
        }

        let effective_search = matched_profiles
            .iter()
            .find_map(|p| p.atspi_override.as_ref())
            .unwrap_or(&atspi_cfg.search);

        let target = service
            .atspi()
            .discover_target_by_pid_with_config(info.pid, effective_search)
            .await?;
        match service.fetch_menu(&target).await {
            Ok(menu) if !menu.children.is_empty() => Some((target, menu, MenuSourceTier::Atspi)),
            _ => {
                // Re-discover once uncached in case a Swing/JVM app rebuilt its JMenuBar under the same PID.
                service.atspi().invalidate_target_cache(info.pid).await;
                let fresh_target = service
                    .atspi()
                    .discover_target_by_pid_with_config(info.pid, effective_search)
                    .await?;
                if fresh_target != target
                    && let Ok(menu) = service.fetch_menu(&fresh_target).await
                    && !menu.children.is_empty()
                {
                    return Some((fresh_target, menu, MenuSourceTier::Atspi));
                }
                None
            }
        }
    }
}
