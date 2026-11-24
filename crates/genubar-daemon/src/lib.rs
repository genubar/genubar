//! Runtime focus coordinator, tiered menu resolution, L1 disk cache, and action execution daemon.

pub mod action_dispatch;
pub mod coordinator;
pub mod executor;
pub mod fallback;
pub mod models;
pub mod resolver;
pub mod state;
pub mod submenu_service;
pub mod tier_acquirer;

pub use coordinator::FocusCoordinator;
pub use models::{MenuPayload, UiRenderConfig};
pub use state::DaemonCaches;

use genubar_dbus::UnifiedMenuService;
use genubar_dbus::server::{AppMenuRegistrar, WindowTrackerService};
use genubar_protocol::{
    REGISTRAR_BUS_NAME, REGISTRAR_OBJECT_PATH, TRACKER_BUS_NAME, TRACKER_OBJECT_PATH,
};
use std::error::Error;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::export::futures_core::Stream;
use zbus::fdo::DBusProxy;

/// Starts the Genubar daemon D-Bus services (`com.canonical.AppMenu.Registrar` and `org.genubar.WindowTracker`).
pub async fn run_daemon_with_service(service: &UnifiedMenuService) -> Result<(), Box<dyn Error>> {
    let caches = DaemonCaches::load_or_new();
    let current_target = Arc::new(RwLock::new(None));
    let conn = service.connection().clone();

    let coordinator = Arc::new(FocusCoordinator::new(
        caches,
        service.clone(),
        current_target,
    )?);

    let registrar = AppMenuRegistrar::new(conn.clone(), coordinator.clone());
    let tracker = WindowTrackerService::new(coordinator);

    conn.object_server()
        .at(REGISTRAR_OBJECT_PATH, registrar)
        .await?;
    conn.object_server()
        .at(TRACKER_OBJECT_PATH, tracker)
        .await?;

    conn.request_name(REGISTRAR_BUS_NAME).await?;
    conn.request_name(TRACKER_BUS_NAME).await?;

    tracing::info!(
        registrar_bus = REGISTRAR_BUS_NAME,
        registrar_path = REGISTRAR_OBJECT_PATH,
        tracker_bus = TRACKER_BUS_NAME,
        tracker_path = TRACKER_OBJECT_PATH,
        "Genubar daemon running"
    );

    let dbus_proxy = DBusProxy::new(&conn).await?;
    let mut name_lost_stream = dbus_proxy.receive_name_lost().await?;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    let lost_our_name = async {
        while let Some(sig) =
            std::future::poll_fn(|cx| Pin::new(&mut name_lost_stream).poll_next(cx)).await
        {
            if let Ok(args) = sig.args() {
                let name = args.name();
                if name == TRACKER_BUS_NAME || name == REGISTRAR_BUS_NAME {
                    return name.to_string();
                }
            }
        }
        String::new()
    };

    tokio::select! {
        _ = conn.closed() => {
            tracing::info!("Session D-Bus connection closed; shutting down daemon");
        }
        lost = lost_our_name => {
            if !lost.is_empty() {
                tracing::info!(bus_name = %lost, "Lost D-Bus name ownership to another instance; shutting down daemon");
            }
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM; shutting down daemon");
        }
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT; shutting down daemon");
        }
    }

    Ok(())
}

/// Connects to the session D-Bus and runs the Genubar daemon until terminated.
pub async fn run_daemon() -> Result<(), Box<dyn Error>> {
    let service = UnifiedMenuService::new().await?;
    run_daemon_with_service(&service).await
}
