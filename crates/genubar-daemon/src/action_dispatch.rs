use crate::coordinator::FocusCoordinator;
use crate::executor::ActionExecutor;
use genubar_config::ConfigManager;
use genubar_core::{ActiveWindowInfo, CustomAction, MenuProvider, MenuTarget, SystemCommand};
use genubar_dbus::MenuSignalSink;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Executes custom `CustomAction` rules and system commands triggered from menu activation.
pub struct ActionDispatcher;

impl ActionDispatcher {
    pub async fn execute_custom_action(
        coordinator: &FocusCoordinator,
        custom: &CustomAction,
        sink: &dyn MenuSignalSink,
    ) -> bool {
        let info = coordinator.last_window.read().await.clone();

        match custom {
            CustomAction::Command(cmd) => ActionExecutor::spawn_command(cmd, &info),
            CustomAction::Shell(script) => ActionExecutor::spawn_shell(script, &info),
            CustomAction::OpenUri(uri) => ActionExecutor::open_uri(uri, &info),
            // Handled directly by the GNOME Shell extension frontend (clipboard / X11/Wayland synthetic input).
            CustomAction::CopyText(_)
            | CustomAction::InsertText(_)
            | CustomAction::KeyShortcut(_) => true,
            CustomAction::DbusCall {
                dest,
                path,
                interface,
                method,
            } => {
                ActionExecutor::call_dbus(
                    coordinator.service.connection(),
                    dest,
                    path,
                    interface,
                    method,
                )
                .await
            }
            CustomAction::System(sys_cmd) => {
                Self::execute_system_command(coordinator, *sys_cmd, &info, sink).await
            }
            CustomAction::GtkAction(action_spec) => {
                let hooked_bus = match coordinator
                    .caches
                    .windows
                    .resolve_gtk4(info.pid, &info.gtk_bus_name)
                    .await
                {
                    Some(MenuTarget::Gtk { bus_name, .. }) => Some(bus_name),
                    _ => match coordinator.current_target.read().await.clone() {
                        Some(MenuTarget::Gtk { bus_name, .. }) => Some(bus_name),
                        _ => None,
                    },
                };
                coordinator
                    .service
                    .gtk()
                    .activate_custom_gtk_action(&info, hooked_bus.as_deref(), action_spec)
                    .await
            }
        }
    }

    async fn execute_system_command(
        coordinator: &FocusCoordinator,
        sys_cmd: SystemCommand,
        info: &ActiveWindowInfo,
        sink: &dyn MenuSignalSink,
    ) -> bool {
        if let Some(handled) = ActionExecutor::execute_os_system_command(sys_cmd, info.pid) {
            return handled;
        }
        match sys_cmd {
            SystemCommand::CaptureMenuSnapshot => {
                !coordinator.capture_snapshot_to_disk().await.is_empty()
            }
            SystemCommand::RefreshMenu => {
                coordinator
                    .service
                    .atspi()
                    .clear_cache_for_pid(info.pid, None)
                    .await;
                let (target, root, tier) = coordinator.resolve_with_config(info).await;
                let _ = coordinator
                    .publish_state(info, target, root, tier, sink)
                    .await;
                true
            }
            SystemCommand::ReloadConfig => {
                let reloaded = tokio::task::spawn_blocking(|| -> Option<ConfigManager> {
                    let mgr = ConfigManager::load_or_bootstrap().ok()?;
                    mgr.sync_platform_env();
                    Some(mgr)
                })
                .await
                .ok()
                .flatten();

                if let Some(new_cfg) = reloaded {
                    let next_css_rev = new_cfg.stylesheet_fingerprint();
                    coordinator
                        .service
                        .atspi()
                        .set_disk_cache_enabled(new_cfg.settings.api_tiers.atspi.enable_disk_cache);
                    *coordinator.config.write().await = Arc::new(new_cfg);
                    coordinator
                        .css_revision
                        .store(next_css_rev, Ordering::SeqCst);
                    let (target, root, tier) = coordinator.resolve_with_config(info).await;
                    let _ = coordinator
                        .publish_state(info, target, root, tier, sink)
                        .await;
                    return true;
                }
                tracing::warn!("ReloadConfig failed while loading ~/.config/genubar");
                false
            }
            _ => false,
        }
    }

    pub async fn activate_menu_item(
        coordinator: &FocusCoordinator,
        item_id: i32,
        sink: &dyn MenuSignalSink,
    ) -> bool {
        let custom_opt = {
            let guard = coordinator.current_menu.read().await;
            guard
                .as_ref()
                .and_then(|payload| payload.root.find_by_id(item_id))
                .and_then(|node| node.action.as_deref())
                .and_then(CustomAction::parse)
        };

        if let Some(custom) = custom_opt {
            tracing::debug!(item_id, action = ?custom, "Executing custom menu action");
            return Self::execute_custom_action(coordinator, &custom, sink).await;
        }

        let Some(target) = coordinator.current_target.read().await.clone() else {
            return false;
        };

        match coordinator.service.activate_item(&target, item_id).await {
            Ok(()) => {
                tracing::debug!(item_id, "Activated backend menu item");

                let mut toggled_json = None;
                {
                    let mut guard = coordinator.current_menu.write().await;
                    if let Some(ref mut payload) = *guard
                        && payload.root.toggle_check_or_radio_by_id(item_id)
                    {
                        toggled_json = serde_json::to_string(payload).ok();
                    }
                }
                if let Some(json) = toggled_json {
                    sink.emit_active_menu_changed(&json).await;
                }
                true
            }
            Err(err) => {
                tracing::warn!(item_id, error = %err, "Failed to activate backend menu item");
                false
            }
        }
    }
}
