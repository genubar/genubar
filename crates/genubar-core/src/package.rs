use serde::{Deserialize, Serialize};

/// Unified package/component kind across patches, configs, marketplace, and installed catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    #[default]
    Patch,
    Profile,
    Style,
    IconPack,
    Bundle,
    Script,
    #[serde(other)]
    Unknown,
}
