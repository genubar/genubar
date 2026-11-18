use thiserror::Error;

/// Typed errors produced by `genubar-installer` artifact resolution and system setup.
#[derive(Debug, Error)]
pub enum InstallerError {
    #[error("I/O error ({context}): {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Extension operation failed: {0}")]
    Extension(String),

    #[error("External command `{command}` failed: {details}")]
    CommandFailed { command: String, details: String },

    #[error("Required artifact {artifacts:?} not found locally or in release {version}")]
    ArtifactNotFound {
        artifacts: Vec<String>,
        version: String,
    },

    #[error("Background task panicked: {0}")]
    Task(String),
}
