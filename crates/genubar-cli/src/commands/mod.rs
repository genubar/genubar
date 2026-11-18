pub mod daemon;
pub mod inspect;
pub mod packages;

use genubar_dbus::unified::UnifiedMenuService;

pub async fn notify_daemon_reload_config(service: &UnifiedMenuService) {
    let _ = service
        .connection()
        .call_method(
            Some(genubar_protocol::TRACKER_BUS_NAME),
            genubar_protocol::TRACKER_OBJECT_PATH,
            Some(genubar_protocol::TRACKER_INTERFACE),
            "ReloadConfig",
            &(),
        )
        .await;
}
