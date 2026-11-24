use super::proxy::{A11yBusProxy, A11yStatusProxy};
use genubar_core::MenuError;
use std::sync::Arc;
use tokio::sync::OnceCell;
use zbus::{Connection, connection};

/// Lazily initializes and caches the shared AT-SPI2 accessibility bus connection.
#[derive(Clone)]
pub struct A11yBusClient {
    session_conn: Connection,
    a11y_conn: Arc<OnceCell<Connection>>,
}

impl A11yBusClient {
    pub fn new(session_conn: Connection) -> Self {
        Self {
            session_conn,
            a11y_conn: Arc::new(OnceCell::new()),
        }
    }

    pub async fn connection(&self) -> Result<&Connection, MenuError> {
        self.a11y_conn
            .get_or_try_init(|| async {
                if let Ok(status_proxy) = A11yStatusProxy::new(&self.session_conn).await {
                    let _ = status_proxy.set_is_enabled(true).await;
                    let _ = status_proxy.set_screen_reader_enabled(false).await;
                }
                let bus_proxy = A11yBusProxy::new(&self.session_conn)
                    .await
                    .map_err(|e| MenuError::Transport(format!("A11yBus proxy error: {e}")))?;
                let addr = bus_proxy.get_address().await.map_err(|e| {
                    MenuError::Transport(format!("Failed to get A11y address: {e}"))
                })?;
                connection::Builder::address(addr.as_str())
                    .build()
                    .await
                    .map_err(|e| MenuError::Transport(format!("Failed to connect A11y bus: {e}")))
            })
            .await
    }
}
