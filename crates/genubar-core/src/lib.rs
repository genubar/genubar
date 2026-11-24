//! Core domain models, menu tree primitives, compatibility validation, and provider traits for Genubar.

pub mod action;
pub mod compat;
pub mod errors;
pub mod node;
pub mod package;
mod traits;

pub use action::{CustomAction, PatchErrorPolicy, SystemCommand};
pub use compat::{
    CURRENT_APP_VERSION, EXPECTED_EXTENSION_VERSION, EXTENSION_FILES, EXTENSION_UUID,
    ExtensionCompatReport, IPC_PROTOCOL_VERSION, MIN_COMPATIBLE_EXTENSION_VERSION,
    check_extension_metadata_json, is_version_compatible, parse_semver_triple,
};
pub use errors::MenuError;
pub use node::{
    ActiveWindowInfo, AtspiSearchConfig, MenuItemType, MenuNode, MenuSourceTier, MenuTarget,
    clean_label, wildcard_match,
};
pub use package::PackageKind;
pub use traits::MenuProvider;
