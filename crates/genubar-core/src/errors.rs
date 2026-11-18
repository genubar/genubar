use thiserror::Error;

#[derive(Debug, Error)]
pub enum MenuError {
    #[error("Transport or D-Bus error: {0}")]
    Transport(String),
    #[error("Failed to parse menu payload: {0}")]
    ParseError(String),
    #[error("Target application or menu path not found: {0}")]
    NotFound(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Patch condition check failed: {0}")]
    CheckFailed(String),
    #[error("System I/O or permission error: {0}")]
    IoPermission(String),
    /// Returned when a patch contains a command from a newer Genubar version.
    #[error("Unsupported patch command '{0}'. Update Genubar or skip this step.")]
    UnsupportedStep(String),
    /// Returned on rollback when `patch_ledger.json` lacks the original state record.
    #[error("Patch ledger is missing rollback data for '{0}'. Confirmation required for unsafe cleanup.")]
    CorruptedLedger(String),
}