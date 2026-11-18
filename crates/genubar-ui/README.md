# genubar-ui

GTK4 + Libadwaita desktop settings application (`genubar-settings`) for configuring menus, styles, icon packs, application profiles, patches, bundles, updates, and live daemon synchronization.

## Architecture

- **Application Shell (`main.rs`, `window.rs`, `state.rs`):** Manages `AppState` (shared `ConfigManager`, `PatchManager`, `ToastOverlay`, debounced `save_and_reload`, dirty-tab refreshers, and `RegistryCache`).
- **Domain & IPC Services (`services/`):**
  - `config_service/` — debounced saves and style/env synchronization.
  - `daemon_ipc.rs` — notifies `org.genubar.WindowTracker` to `ReloadConfig` or capture snapshots.
  - `cache_service.rs`, `patch_service.rs`, `bundle_service.rs` — L1 cache clearing, patch apply/revert with confirmation modals, and composite bundle toggles.
- **Reusable UI Components (`components/`, `utils/`, `models/`):** Confirm modals, code/text editor dialogs, exclusions manager, script selector rows, and Pango markup escaping (`text.rs`).
- **11 Navigation Views (`views/`):**
  - `general_view.rs` — top-panel overflow (`More`), max columns, Python engine, factory resets.
  - `styles_view/` — style preset management, theme mode (`Auto`/`Dark`/`Light`), GNOME panel recoloring, pixel metrics injection.
  - `icons_view/` — icon column/check/radio policies and custom icon packs.
  - `system_menus_view/` — root column sorting and `app_menu.toml` / `desktop_menu.toml` editors.
  - `profiles_view/` — application matcher editor, inheritance column builder, snapshot capture/import.
  - `patches_view/` — system app detection, `Strict`/`Ignore` error policies, apply/revert cards.
  - `protocols_view/` — per-tier toggles (GTK Native, GTK4 Hook, Canonical DBusMenu, AT-SPI2) and L1 cache controls.
  - `bundles_view/` — expandable bundle cards with live component status and one-click `Enable All` / `Disable All`.
  - `marketplace_view/` — remote `registry.json` catalog browser with `min_app_version` compatibility guards.
  - `installed_view.rs` — searchable inventory of all installed packages and scripts.
  - `about_view/` — bidirectional Core Daemon <-> GNOME Extension (`genubar@github.com`) version check and updater.

## Running

```bash
cargo run -p genubar-ui
```
