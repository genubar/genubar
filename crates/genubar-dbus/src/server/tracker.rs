use super::{MenuSignalSink, TrackerBackend};
use async_trait::async_trait;
use genubar_core::ActiveWindowInfo;
use std::sync::Arc;
use zbus::interface;
use zbus::object_server::SignalEmitter;

struct ZbusMenuSignalSink<'a>(&'a SignalEmitter<'a>);

#[async_trait]
impl MenuSignalSink for ZbusMenuSignalSink<'_> {
    async fn emit_active_menu_changed(&self, payload_json: &str) {
        let _ = WindowTrackerService::active_menu_changed(self.0, payload_json).await;
    }
}

/// Thin zbus `org.genubar.WindowTracker` transport adapter delegating to [`TrackerBackend`].
pub struct WindowTrackerService {
    backend: Arc<dyn TrackerBackend>,
}

impl WindowTrackerService {
    pub fn new(backend: Arc<dyn TrackerBackend>) -> Self {
        Self { backend }
    }
}

#[interface(name = "org.genubar.WindowTracker")]
impl WindowTrackerService {
    #[zbus(signal)]
    pub(super) async fn active_menu_changed(
        emitter: &SignalEmitter<'_>,
        payload_json: &str,
    ) -> zbus::Result<()>;

    async fn get_active_menu(&self) -> String {
        self.backend.get_active_menu_json().await
    }

    async fn reload_config(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> bool {
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend.reload_config(&sink).await
    }

    async fn clear_caches(&self, clear_l1: bool, clear_l2: bool) -> bool {
        self.backend.clear_caches(clear_l1, clear_l2).await
    }

    async fn capture_active_snapshot(&self) -> String {
        self.backend.capture_active_snapshot().await
    }

    async fn get_daemon_info(&self) -> (String, String, u32) {
        (
            genubar_core::CURRENT_APP_VERSION.to_string(),
            genubar_core::EXPECTED_EXTENSION_VERSION.to_string(),
            genubar_core::IPC_PROTOCOL_VERSION,
        )
    }

    async fn fetch_submenu(&self, item_id: i32) -> String {
        self.backend.fetch_submenu_json(item_id).await
    }

    async fn activate_item(
        &self,
        item_id: i32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> bool {
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend.activate_item(item_id, &sink).await
    }

    async fn register_gtk4_window(
        &self,
        pid: u32,
        bus_name: String,
        menubar_path: String,
        app_path: String,
        win_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend
            .register_gtk4_window(pid, bus_name, menubar_path, app_path, win_path, &sink)
            .await;
    }

    async fn refresh_canonical_window(
        &self,
        window_id: u32,
        pid: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend
            .refresh_canonical_window(window_id, pid, &sink)
            .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify_focus_changed(
        &self,
        pid: u32,
        xid: u32,
        title: String,
        app_id: String,
        gtk_bus_name: String,
        gtk_menubar_path: String,
        gtk_app_menu_path: String,
        gtk_app_path: String,
        gtk_win_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        let info = ActiveWindowInfo {
            pid,
            xid,
            title,
            app_id,
            gtk_bus_name,
            gtk_menubar_path,
            gtk_app_menu_path,
            gtk_app_path,
            gtk_win_path,
        };
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend.notify_focus_changed(info, &sink).await
    }

    async fn create_profile_from_snapshot(
        &self,
        snapshot_file: String,
        sync_new_items: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        let sink = ZbusMenuSignalSink(&emitter);
        self.backend
            .create_profile_from_snapshot(snapshot_file, sync_new_items, &sink)
            .await
    }
}
