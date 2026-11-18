use crate::error::UiError;
use genubar_protocol::{TRACKER_BUS_NAME, TRACKER_INTERFACE, TRACKER_OBJECT_PATH};

pub struct DaemonIpcService;

fn run_zbus_call<T, F>(call: F) -> Result<T, UiError>
where
    F: std::future::Future<Output = zbus::Result<T>>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|source| UiError::Io {
            context: String::from("create Tokio runtime for D-Bus IPC"),
            source,
        })?;
    rt.block_on(call).map_err(|e| UiError::Ipc(e.to_string()))
}

impl DaemonIpcService {
    /// Triggers `ReloadConfig` on the running `org.genubar.WindowTracker` D-Bus service.
    pub fn reload_daemon() -> bool {
        let res = run_zbus_call(async {
            let conn = zbus::Connection::session().await?;
            conn.call_method(
                Some(TRACKER_BUS_NAME),
                TRACKER_OBJECT_PATH,
                Some(TRACKER_INTERFACE),
                "ReloadConfig",
                &(),
            )
            .await
            .map(|_| ())
        });
        if let Err(ref err) = res {
            tracing::debug!(error = %err, "Daemon ReloadConfig D-Bus call failed");
        }
        res.is_ok()
    }

    /// Asks the running daemon to clear its in-memory & on-disk L1/L2 caches via D-Bus.
    pub fn clear_caches(clear_l1: bool, clear_l2: bool) -> bool {
        let res = run_zbus_call(async {
            let conn = zbus::Connection::session().await?;
            let reply = conn
                .call_method(
                    Some(TRACKER_BUS_NAME),
                    TRACKER_OBJECT_PATH,
                    Some(TRACKER_INTERFACE),
                    "ClearCaches",
                    &(clear_l1, clear_l2),
                )
                .await?;
            let ok: bool = reply.body().deserialize().unwrap_or(true);
            Ok(ok)
        });
        match res {
            Ok(ok) => ok,
            Err(err) => {
                tracing::debug!(error = %err, "Daemon ClearCaches D-Bus call unavailable");
                false
            }
        }
    }
}
