use genubar_core::MenuError;
use thiserror::Error;

/// Typed errors across `genubar-ui` services, configuration persistence, and background tasks.
#[derive(Debug, Error)]
pub enum UiError {
    #[error("{0}")]
    Domain(#[from] MenuError),

    #[error("I/O error ({context}): {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("TOML syntax error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("D-Bus IPC error: {0}")]
    Ipc(String),

    #[error("Background task panicked: {0}")]
    Task(String),
}
