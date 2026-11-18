use crate::action_dispatch::ActionDispatcher;
use crate::fallback::FallbackMenuBuilder;
use crate::models::{MenuPayload, UiRenderConfig};
use crate::resolver::WindowMenuResolver;
use crate::state::DaemonCaches;
use crate::submenu_service::SubmenuService;
use async_trait::async_trait;
use genubar_config::ConfigManager;
use genubar_core::{
    ActiveWindowInfo, CustomAction, MenuError, MenuNode, MenuSourceTier, MenuTarget, SystemCommand,
};
use genubar_dbus::{MenuSignalSink, RegisteredWindow, RegistrarBackend, TrackerBackend, UnifiedMenuService};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Coordinates window focus switches, two-stage cache/live menu resolution, and D-Bus publication.
pub struct FocusCoordinator {
    pub caches: DaemonCaches,
    pub service: UnifiedMenuService,
    pub config: Arc<RwLock<Arc<ConfigManager>>>,
    pub current_target: Arc<RwLock<Option<MenuTarget>>>,
    pub current_menu: Arc<RwLock<Option<MenuPayload>>>,
    pub last_window: Arc<RwLock<ActiveWindowInfo>>,
    pub focus_seq: Arc<AtomicU64>,
    pub css_revision: Arc<AtomicU64>,
    pub last_published_json: Arc<RwLock<String>>,
}

impl FocusCoordinator {
    pub fn new(
        caches: DaemonCaches,
        service: UnifiedMenuService,
        current_target: Arc<RwLock<Option<MenuTarget>>>,
    ) -> Result<Self, MenuError> {
        let mgr = ConfigManager::load_or_bootstrap()?;
        Self::from_config_manager(caches, service, current_target, mgr)
    }

    pub fn from_config_manager(
        caches: DaemonCaches,
        service: UnifiedMenuService,
        current_target: Arc<RwLock<Option<MenuTarget>>>,
        mgr: ConfigManager,
    ) -> Result<Self, MenuError> {
        mgr.sync_platform_env();
        service
            .atspi()
            .set_disk_cache_enabled(mgr.settings.api_tiers.atspi.enable_disk_cache);

        Ok(Self {
            caches,
            service,
            config: Arc::new(RwLock::new(Arc::new(mgr))),
            current_target,
            current_menu: Arc::new(RwLock::new(None)),
            last_window: Arc::new(RwLock::new(ActiveWindowInfo::default())),
            focus_seq: Arc::new(AtomicU64::new(0)),
            css_revision: Arc::new(AtomicU64::new(1)),
            last_published_json: Arc::new(RwLock::new(String::new())),
        })
    }

    async fn config_snapshot(&self) -> Arc<ConfigManager> {
        Arc::clone(&*self.config.read().await)
    }

    pub async fn build_ui_render_config(&self) -> UiRenderConfig {
        let cfg = self.config_snapshot().await;
        let icon_cfg = &cfg.icons.general;
        UiRenderConfig {
            show_menu_icons: icon_cfg.show_menu_icons && cfg.settings.general.show_menu_icons,
            reserve_icon_column: icon_cfg.reserve_icon_column,
            icon_column_policy: icon_cfg.icon_column_policy,
            check_position: icon_cfg.check_position,
            check_column_policy: icon_cfg.check_column_policy,
            check_style: icon_cfg.check_style,
            icon_size_px: icon_cfg.icon_size_px,
            theme_mode: cfg.settings.style.theme_mode,
            recolor_gnome_panel: cfg.settings.style.recolor_gnome_panel,
            css_revision: self.css_revision.load(Ordering::SeqCst),
        }
    }

    pub async fn resolve_with_config(
        &self,
        info: &ActiveWindowInfo,
    ) -> (Option<MenuTarget>, MenuNode, MenuSourceTier) {
        let cfg = self.config_snapshot().await;
        WindowMenuResolver::resolve_and_fetch(&self.service, &self.caches, &cfg, info).await
    }

    pub async fn capture_snapshot_to_disk(&self) -> String {
        let info = self.last_window.read().await.clone();
        let payload_opt = self.current_menu.read().await.clone();
        if let Some(payload) = payload_opt {
            let cfg = self.config_snapshot().await;
            let saved = tokio::task::spawn_blocking(move || {
                cfg.snapshots
                    .save_snapshot(&info, MenuSourceTier::AppProfile, &payload.root)
            })
            .await;

            if let Ok(Ok(path)) = saved {
                tracing::info!(path = %path.display(), "Saved active menu snapshot");
                return path.to_string_lossy().to_string();
            }
        }
        String::new()
    }

    pub async fn publish_state(
        &self,
        info: &ActiveWindowInfo,
        target: Option<MenuTarget>,
        root: MenuNode,
        tier: MenuSourceTier,
        sink: &dyn MenuSignalSink,
    ) -> String {
        let source_str = tier.as_wire_str();
        let top_labels: Vec<String> = root
            .children
            .iter()
            .filter(|n| n.visible)
            .map(|n| n.clean_label())
            .collect();

        tracing::info!(
            window = if info.pid == 0 { "Desktop" } else { &info.title },
            pid = info.pid,
            source = source_str,
            columns = %top_labels.join(" | "),
            "Published active menu state"
        );

        let ui_config = self.build_ui_render_config().await;
        let payload = MenuPayload {
            app_title: FallbackMenuBuilder::clean_app_name(info),
            app_id: info.app_id.clone(),
            pid: info.pid,
            source: source_str.to_string(),
            ui_config,
            root,
        };

        *self.current_target.write().await = target;
        *self.current_menu.write().await = Some(payload.clone());

        if let Ok(json) = serde_json::to_string(&payload) {
            let mut last_guard = self.last_published_json.write().await;
            if *last_guard != json {
                *last_guard = json.clone();
                drop(last_guard);
                sink.emit_active_menu_changed(&json).await;
            }
            return json;
        }
        String::new()
    }
}

#[async_trait]
impl RegistrarBackend for FocusCoordinator {
    async fn register_window(&self, entry: RegisteredWindow) {
        self.caches.windows.insert_canonical(entry).await;
    }

    async fn unregister_window(&self, window_id: u32) -> bool {
        self.caches.windows.remove_canonical(window_id).await.is_some()
    }

    async fn resolve_canonical(&self, window_id: u32) -> Option<MenuTarget> {
        self.caches.windows.resolve_canonical(window_id, 0).await
    }

    async fn notify_canonical_registered(
        &self,
        window_id: u32,
        pid: u32,
        sink: &dyn MenuSignalSink,
    ) {
        self.refresh_canonical_window(window_id, pid, sink).await;
    }
}

#[async_trait]
impl TrackerBackend for FocusCoordinator {
    async fn get_active_menu_json(&self) -> String {
        if let Some(ref payload) = *self.current_menu.read().await {
            serde_json::to_string(payload).unwrap_or_default()
        } else {
            let ui_config = self.build_ui_render_config().await;
            let cfg = self.config_snapshot().await;
            let cfg_blocking = Arc::clone(&cfg);
            let desktop_info = ActiveWindowInfo::default();
            let app_title = cfg.desktop_menu.resolved_system_title();
            let root = tokio::task::spawn_blocking(move || {
                WindowMenuResolver::resolve_desktop_menu(&cfg_blocking, &desktop_info)
            })
            .await
            .unwrap_or_else(|_| {
                FallbackMenuBuilder::build_desktop_menu(&cfg, &ActiveWindowInfo::default())
            });

            let payload = MenuPayload {
                app_title,
                app_id: String::new(),
                pid: 0,
                source: MenuSourceTier::DesktopIdle.as_wire_str().to_string(),
                ui_config,
                root,
            };
            serde_json::to_string(&payload).unwrap_or_default()
        }
    }

    async fn reload_config(&self, sink: &dyn MenuSignalSink) -> bool {
        ActionDispatcher::execute_custom_action(
            self,
            &CustomAction::System(SystemCommand::ReloadConfig),
            sink,
        )
        .await
    }

    async fn clear_caches(&self, clear_l1: bool, clear_l2: bool) -> bool {
        if clear_l1 {
            self.caches.l1.clear_all().await;
            self.caches.canonical.clear_all().await;
        }
        if clear_l2 {
            self.service.atspi().clear_all_caches().await;
        }
        if clear_l1 || clear_l2 {
            self.last_published_json.write().await.clear();
            tracing::info!(clear_l1, clear_l2, "Cleared daemon menu caches (RAM + disk)");
        }
        true
    }

    async fn capture_active_snapshot(&self) -> String {
        self.capture_snapshot_to_disk().await
    }

    async fn fetch_submenu_json(&self, item_id: i32) -> String {
        let Some(target) = self.current_target.read().await.clone() else {
            return String::new();
        };
        let info = self.last_window.read().await.clone();
        let cfg = self.config_snapshot().await;

        let processed = SubmenuService::fetch_and_process_submenu(
            &self.service,
            &self.caches,
            &cfg,
            &info,
            &target,
            &self.current_menu,
            item_id,
        )
        .await;

        processed
            .and_then(|node| serde_json::to_string(&node).ok())
            .unwrap_or_default()
    }

    async fn activate_item(&self, item_id: i32, sink: &dyn MenuSignalSink) -> bool {
        ActionDispatcher::activate_menu_item(self, item_id, sink).await
    }

    async fn register_gtk4_window(
        &self,
        pid: u32,
        bus_name: String,
        menubar_path: String,
        app_path: String,
        win_path: String,
        sink: &dyn MenuSignalSink,
    ) {
        let target = MenuTarget::Gtk {
            bus_name: bus_name.clone(),
            menubar_path: Some(menubar_path.clone()),
            app_menu_path: None,
            app_path: Some(app_path),
            win_path: Some(win_path),
        };

        self.caches.windows.insert_gtk4(pid, target.clone()).await;
        tracing::info!(pid, dest = %bus_name, path = %menubar_path, "Registered GTK4-Hook window");

        let info = self.last_window.read().await.clone();
        let is_current_window = info.pid == pid
            || (!bus_name.is_empty() && info.gtk_bus_name == bus_name)
            || (pid <= 10 && info.pid > 10 && info.gtk_bus_name.is_empty());

        if is_current_window {
            if info.pid != 0 && info.pid != pid {
                self.caches.windows.insert_gtk4(info.pid, target).await;
            }
            self.focus_seq.fetch_add(1, Ordering::SeqCst);
            let (resolved_target, root_menu, tier) = self.resolve_with_config(&info).await;
            let _ = self
                .publish_state(&info, resolved_target, root_menu, tier, sink)
                .await;
        }
    }

    async fn refresh_canonical_window(
        &self,
        window_id: u32,
        pid: u32,
        sink: &dyn MenuSignalSink,
    ) {
        let info = self.last_window.read().await.clone();
        let is_active =
            (window_id != 0 && info.xid == window_id) || (pid != 0 && info.pid == pid);

        if is_active {
            self.focus_seq.fetch_add(1, Ordering::SeqCst);
            let (resolved_target, root_menu, tier) = self.resolve_with_config(&info).await;
            let _ = self
                .publish_state(&info, resolved_target, root_menu, tier, sink)
                .await;
        }
    }

    async fn notify_focus_changed(
        &self,
        info: ActiveWindowInfo,
        sink: &dyn MenuSignalSink,
    ) -> String {
        let seq = self.focus_seq.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_window.write().await = info.clone();

        // Stage 1: Instant L1 / RAM Cache emission (<1ms)
        {
            let cfg = self.config_snapshot().await;
            if let Some((fast_target, fast_root, fast_tier)) =
                WindowMenuResolver::resolve_fast_cached_menu(&self.caches, &cfg, &info).await
            {
                if self.focus_seq.load(Ordering::SeqCst) != seq {
                    return self.get_active_menu_json().await;
                }
                if let Some(ref target @ MenuTarget::Canonical { .. }) = fast_target {
                    self.service
                        .canonical()
                        .seed_target_paths_from_menu(target, &fast_root)
                        .await;
                }
                let _ = self
                    .publish_state(&info, fast_target, fast_root, fast_tier, sink)
                    .await;
            }
        }

        if self.focus_seq.load(Ordering::SeqCst) != seq {
            return self.get_active_menu_json().await;
        }

        // Stage 2: Live verification / resolution
        let (resolved_target, root_menu, tier) = self.resolve_with_config(&info).await;

        if self.focus_seq.load(Ordering::SeqCst) != seq {
            return self.get_active_menu_json().await;
        }

        self.publish_state(&info, resolved_target, root_menu, tier, sink)
            .await
    }

    async fn create_profile_from_snapshot(
        &self,
        snapshot_file: String,
        sync_new_items: bool,
        sink: &dyn MenuSignalSink,
    ) -> String {
        let cfg = self.config_snapshot().await;
        let path_out = tokio::task::spawn_blocking(move || {
            let profiles_dir = cfg.base_dir().join("profiles");
            cfg.snapshots
                .create_profile_from_snapshot(
                    std::path::Path::new(&snapshot_file),
                    &profiles_dir,
                    sync_new_items,
                )
                .ok()
        })
        .await
        .ok()
        .flatten();

        if let Some(created_path) = path_out {
            let _ = self.reload_config(sink).await;
            return created_path.to_string_lossy().to_string();
        }
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::MenuItemType;
    use std::fs;
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct MockSink {
        emitted: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl MenuSignalSink for MockSink {
        async fn emit_active_menu_changed(&self, payload_json: &str) {
            self.emitted.lock().await.push(payload_json.to_string());
        }
    }

    #[tokio::test]
    async fn focus_coordinator_builds_ui_config_and_deduplicates_identical_publications() {
        let Ok(service) = UnifiedMenuService::new().await else {
            return;
        };

        let tmp_base = std::env::temp_dir().join("genubar_test_coordinator_pub");
        let _ = fs::remove_dir_all(&tmp_base);
        let cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let caches = DaemonCaches {
            windows: Default::default(),
            canonical: Default::default(),
            l1: crate::state::L1SkeletonCache::load_from_path(tmp_base.join("l1.json")),
        };
        let coord = FocusCoordinator::from_config_manager(
            caches,
            service,
            Arc::new(RwLock::new(None)),
            cfg,
        )
        .unwrap();

        let sink = MockSink::default();
        let info = ActiveWindowInfo {
            pid: 777,
            xid: 888,
            app_id: String::from("org.example.App"),
            title: String::from("Doc - App"),
            ..Default::default()
        };
        let root = MenuNode {
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

        let first = coord
            .publish_state(&info, None, root.clone(), MenuSourceTier::AppFallback, &sink)
            .await;
        let second = coord
            .publish_state(&info, None, root.clone(), MenuSourceTier::AppFallback, &sink)
            .await;
        assert_eq!(first, second);
        assert_eq!(sink.emitted.lock().await.len(), 1);

        // Clearing caches must clear last_published_json so an identical republish notifies listeners.
        assert!(coord.clear_caches(true, true).await);
        let third = coord
            .publish_state(&info, None, root, MenuSourceTier::AppFallback, &sink)
            .await;
        assert_eq!(third, first);
        assert_eq!(sink.emitted.lock().await.len(), 2);

        // When current_menu is empty, get_active_menu_json falls back to Desktop-Idle.
        *coord.current_menu.write().await = None;
        let desktop_json: serde_json::Value =
            serde_json::from_str(&coord.get_active_menu_json().await).unwrap();
        assert_eq!(desktop_json["source"], "Desktop-Idle");
        assert!(!desktop_json["root"]["children"].as_array().unwrap().is_empty());

        let _ = fs::remove_dir_all(&tmp_base);
    }
}
