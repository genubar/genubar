pub mod registrar;
pub mod tracker;

use crate::models::RegisteredWindow;
use async_trait::async_trait;
use genubar_core::{ActiveWindowInfo, MenuTarget};

/// Port implemented by the daemon registry for `com.canonical.AppMenu.Registrar`.
#[async_trait]
pub trait RegistrarBackend: Send + Sync {
    async fn register_window(&self, entry: RegisteredWindow);
    async fn unregister_window(&self, window_id: u32) -> bool;
    async fn resolve_canonical(&self, window_id: u32) -> Option<MenuTarget>;
    async fn notify_canonical_registered(
        &self,
        window_id: u32,
        pid: u32,
        sink: &dyn MenuSignalSink,
    );
}

/// Abstract emitter for `org.genubar.WindowTracker::ActiveMenuChanged` signals.
#[async_trait]
pub trait MenuSignalSink: Send + Sync {
    async fn emit_active_menu_changed(&self, payload_json: &str);
}

/// Port implemented by the daemon backend for `org.genubar.WindowTracker`.
#[async_trait]
pub trait TrackerBackend: Send + Sync {
    async fn get_active_menu_json(&self) -> String;
    async fn reload_config(&self, sink: &dyn MenuSignalSink) -> bool;
    async fn clear_caches(&self, clear_l1: bool, clear_l2: bool) -> bool;
    async fn capture_active_snapshot(&self) -> String;
    async fn fetch_submenu_json(&self, item_id: i32) -> String;
    async fn activate_item(&self, item_id: i32, sink: &dyn MenuSignalSink) -> bool;
    async fn register_gtk4_window(
        &self,
        pid: u32,
        bus_name: String,
        menubar_path: String,
        app_path: String,
        win_path: String,
        sink: &dyn MenuSignalSink,
    );
    async fn refresh_canonical_window(&self, window_id: u32, pid: u32, sink: &dyn MenuSignalSink);
    async fn notify_focus_changed(
        &self,
        info: ActiveWindowInfo,
        sink: &dyn MenuSignalSink,
    ) -> String;
    async fn create_profile_from_snapshot(
        &self,
        snapshot_file: String,
        sync_new_items: bool,
        sink: &dyn MenuSignalSink,
    ) -> String;
}

pub use registrar::AppMenuRegistrar;
pub use tracker::WindowTrackerService;
