# genubar-packages

Bundle orchestration, installed package inventory, remote GitHub update checks, and marketplace catalog installer for Genubar.

## Responsibility

`genubar-packages` coordinates package lifecycle and remote metadata workflows consumed by both `genubar-cli` and `genubar-settings` (`genubar-ui`):

- **Bundle Orchestration (`bundles/orchestrator.rs`, `inspector.rs`, `applier.rs`, `manager.rs`):** Inspects composite bundles (`BundleManifest` in `~/.config/genubar/bundles/`), evaluates live status across bundled patches, profiles, styles, and icon packs, and provides atomic `enable` / `disable` operations (`BundleOrchestrator::set_bundle_enabled`).
- **Update Checking (`update/mod.rs`, `update/fetcher.rs`):** Fetches `app.json` from the configured `app_update_url` (defaulting to `https://raw.githubusercontent.com/genubar/genubar-registry/main/app.json`), checks bidirectional compatibility between `genubar-daemon` and `genubar@github.com`, inspects `systemd` user service status (`InstalledDaemonInfo`), and downloads updated extension files directly from GitHub (`UpdateService::update_extension_files`).
- **Marketplace Catalog & Installer (`installer/mod.rs`, `installer/search.rs`, `models.rs`):** Loads local `~/.config/genubar/example_registry.json` and remote `registry.json` catalogs (`RemoteRegistryCatalog`), validates `min_app_version` compatibility and unknown `PackageKind` entries safely, and installs/uninstalls community packages (`RegistryPackageInstaller`).
- **Installed Inventory (`installed.rs`):** Aggregates all user-installed bundles, patches, profiles, styles, icon packs, and Python scripts (`collect_all_installed_items`, `delete_installed_item`).

## Testing

```bash
cargo test -p genubar-packages
```
