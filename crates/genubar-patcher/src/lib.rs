//! Declarative application patcher with conditional checks, transactional rollback ledger, and multi-format appliers.

pub mod applier;
pub mod condition;
pub mod detector;
pub mod ledger;
pub mod manager;
pub mod models;
pub mod repository;
pub mod utils;

pub use applier::StepApplier;
pub use condition::{ConditionEvaluator, ConditionFailAction, PatchCondition};
pub use detector::SystemDetector;
pub use genubar_core::{CURRENT_APP_VERSION, is_version_compatible};
pub use ledger::{PatchLedger, ReleaseOutcome, ResourceRecord};
pub use manager::PatchManager;
pub use models::{
    AppDetectionRule, ConflictReport, ConflictStrategy, PackageKind, PackageMetadata,
    PackageOrigin, PackageStatus, PatchCardView, PatchCatalogFile, PatchManifest, PatchStep,
};
pub use repository::PatchRepository;
