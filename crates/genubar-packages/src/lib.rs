//! Bundle management, remote registry discovery, app/extension update checking, and package installation.

pub mod bundles;
pub mod installed;
pub mod installer;
pub mod models;
pub mod update;

pub use bundles::{
    BundleActionOutcome, BundleComponentItem, BundleDetailedView, BundleManager,
    BundleOrchestrator,
};
pub use genubar_core::PackageKind;
pub use installed::{collect_all_installed_items, delete_installed_item};
pub use installer::RegistryPackageInstaller;
pub use models::{
    AppReleaseManifest, AppUpdateInfo, CatalogSearchItem, ExtensionReleaseManifest,
    FetchedPackagePayload, InstalledDaemonInfo, InstalledExtensionInfo, InstalledPackageItem,
    RegistrySourceStatus, RemotePackageEntry, RemoteRegistryCatalog, UpdateCheckStatus,
};
pub use update::UpdateService;
