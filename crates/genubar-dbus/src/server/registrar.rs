use super::{MenuSignalSink, RegistrarBackend};
use crate::models::RegisteredWindow;
use async_trait::async_trait;
use genubar_core::MenuTarget;
use std::sync::Arc;
use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, OwnedObjectPath, interface};

struct ConnectionTrackerSignalSink(Connection);

#[async_trait]
impl MenuSignalSink for ConnectionTrackerSignalSink {
    async fn emit_active_menu_changed(&self, payload_json: &str) {
        if let Ok(emitter) = SignalEmitter::new(&self.0, genubar_protocol::TRACKER_OBJECT_PATH)
            && let Err(err) =
                super::tracker::WindowTrackerService::active_menu_changed(&emitter, payload_json)
                    .await
        {
            tracing::warn!(
                error = %err,
                "Failed emitting ActiveMenuChanged after canonical window registration"
            );
        }
    }
}

pub struct AppMenuRegistrar {
    connection: Connection,
    backend: Arc<dyn RegistrarBackend>,
}

impl AppMenuRegistrar {
    pub fn new(connection: Connection, backend: Arc<dyn RegistrarBackend>) -> Self {
        Self {
            connection,
            backend,
        }
    }
}

#[interface(name = "com.canonical.AppMenu.Registrar")]
impl AppMenuRegistrar {
    async fn register_window(
        &self,
        window_id: u32,
        menu_object_path: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
    ) {
        let Some(sender) = header.sender() else {
            return;
        };
        let bus_name = sender.to_string();
        let object_path = menu_object_path.to_string();

        let pid = if let Ok(dbus_proxy) = DBusProxy::new(&self.connection).await {
            let name = BusName::from(sender.clone());
            dbus_proxy.get_connection_unix_process_id(name).await.ok()
        } else {
            None
        };

        let entry = RegisteredWindow {
            window_id,
            pid,
            bus_name: bus_name.clone(),
            object_path: object_path.clone(),
        };
        self.backend.register_window(entry).await;
        tracing::info!(
            window_id,
            pid = ?pid,
            bus_name = %bus_name,
            object_path = %object_path,
            "Registered Canonical AppMenu window"
        );

        let backend = Arc::clone(&self.backend);
        let conn = self.connection.clone();
        let pid_val = pid.unwrap_or(0);
        tokio::spawn(async move {
            let sink = ConnectionTrackerSignalSink(conn);
            backend
                .notify_canonical_registered(window_id, pid_val, &sink)
                .await;
        });
    }

    async fn unregister_window(&self, window_id: u32) {
        if self.backend.unregister_window(window_id).await {
            tracing::info!(window_id, "Unregistered Canonical AppMenu window");
        }
    }

    async fn get_menu_for_window(
        &self,
        window_id: u32,
    ) -> zbus::fdo::Result<(String, OwnedObjectPath)> {
        if let Some(MenuTarget::Canonical {
            bus_name,
            object_path,
        }) = self.backend.resolve_canonical(window_id).await
        {
            let path = OwnedObjectPath::try_from(object_path)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
            Ok((bus_name, path))
        } else {
            Err(zbus::fdo::Error::Failed(format!(
                "No menu registered for window {window_id}"
            )))
        }
    }
}
