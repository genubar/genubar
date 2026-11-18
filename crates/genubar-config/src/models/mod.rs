pub mod bundle;
pub mod icons;
pub mod menu;
pub mod profile;
pub mod script;
pub mod settings;
pub mod snapshot;
pub mod style;

pub use genubar_core::compat::{
    CURRENT_APP_VERSION, EXPECTED_EXTENSION_VERSION, IPC_PROTOCOL_VERSION,
    MIN_COMPATIBLE_EXTENSION_VERSION, is_version_compatible,
};
