//! Persistent configuration models, TOML/CSS defaults, and XDG config repository management for Genubar.

pub mod defaults;
pub mod manager;
pub mod models;
pub mod repos;
pub mod services;
pub mod utils;

pub use manager::ConfigManager;
pub use models::{
    bundle::{BundleManifest, BundleStatusView},
    icons::{
        CheckPosition, CheckVisualStyle, ColumnReservePolicy, IconGeneralConfig, IconMatchRule,
        IconPackManifest, IconPathPattern, IconsConfig,
    },
    menu::{
        AppMenuTemplateConfig, AppTemplateMode, DesktopMenuConfig, DesktopMenuMode, MenuNodeDef,
    },
    profile::{AppMatcher, AppProfile, MatchField},
    script::ScriptMetadata,
    settings::{
        ApiTiersConfig, AtspiTierConfig, CanonicalDbusTierConfig, CssSyncMode, GeneralConfig,
        GlobalSettings, Gtk4HookTierConfig, Gtk4LayoutAlgorithm, GtkNativeTierConfig,
        RootSortConfig, RootSortMode, StyleFileConfig, ThemeMode,
    },
    snapshot::MenuSnapshot,
    style::StyleMetadata,
};
pub use repos::{IconPackRepo, ProfileRepo, SettingsRepo, TemplatesRepo};
pub use services::{snapshot_service::SnapshotStore, styles_service::StylesManager};
