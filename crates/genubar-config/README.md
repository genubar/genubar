# genubar-config

Persistent configuration manager, data models, embedded factory defaults, and XDG file repositories for `~/.config/genubar/`.

## Responsibility

`genubar-config` manages everything stored in the user's configuration directory:

- **Global Settings (`models/settings.rs`, `defaults/settings.toml`):** General behavior (`app_update_url`, `registry_urls`, `python_binary`, `script_timeout_ms`, `enable_more_overflow`, `max_top_columns`, `more_column_label`, `patch_error_policy`, `enable_menu_search`, `show_menu_icons`, `auto_map_standard_icons`), style metrics and theme mode (`ThemeMode`, `CssSyncMode`), root column sorting (`RootSortConfig`), and per-tier API configuration (`ApiTiersConfig`).
- **System & App Menu Templates (`models/menu.rs`, `defaults/app_menu.toml`, `defaults/desktop_menu.toml`):** Universal top-bar menu templates and Desktop Idle mode menu trees with `{os_name}` / `{app_name}` placeholder support.
- **Application Profiles (`models/profile.rs`, `defaults/profiles/*.toml`):** Matcher rules (`AppMatcher` against `app_id`, `title`, `any`, `wildcard`, or `desktop`) and declarative column inheritance definitions.
- **Icons & Styles (`models/icons.rs`, `models/style.rs`, `defaults/icons.toml`, `defaults/style.css`):** Icon sizing, check/radio indicators, system fallback rules, custom icon packs (`pack.toml`), and CSS style sheets with `==GenubarStyle==` headers.
- **Scripts, Snapshots, Bundles (`models/script.rs`, `models/snapshot.rs`, `models/bundle.rs`):** Metadata models and `SnapshotStore` / `StylesManager` services.
- **Bootstrapping & Reset (`defaults.rs`):** `ConfigBootstrapper::bootstrap_if_missing` populates default files on first run without overwriting user customizations, while `ConfigManager` exposes targeted reset methods (`reset_settings_to_defaults`, `reset_styles_to_defaults`, `reset_icons_to_defaults`, `reset_system_menus_to_defaults`, `reset_profiles_to_defaults`, `reset_all_to_factory_defaults`).

## Self-Documenting Defaults & CSS Widget Map

Every embedded template under `defaults/` includes inline option tables and examples for end users:

- **`defaults/style.css`:** Includes a complete ASCII diagram of GNOME Shell's St / Clutter widget hierarchy (`#panel.genubar-panel-light`, `.genubar-panel-button`, `.genubar-popup-content`, `.genubar-menu-scroll`, `.genubar-submenu-scroll`, `.genubar-item-row`, `-disabled`, `-active`, `.genubar-search-entry`, etc.) so theme authors can target every component without guessing.
- **`defaults/settings.toml`, `desktop_menu.toml`, `app_menu.toml`, `icons.toml`, `sample_pack.toml`:** Document all supported enum options, policies, placeholders (`{os_name}`, `{app_name}`), and action prefixes (`gtk:`, `sys:`, `key:`, `copy:`, `insert:`, `cmd:`, `sh:`, `uri:`, `dbus:`).
- **`defaults/profiles/*.toml`, `patches/*.toml`, `bundles/*/bundle.toml`:** Serve as ready-to-copy templates demonstrating profile matchers, inheritance (`inherit` / `inherit_others`), `sync_new_items`, patch steps, and bundles.

## Module Structure

```text
src/
├── lib.rs         # Public re-exports (ConfigManager, models, repos, services)
├── manager.rs     # ConfigManager facade loading/saving/syncing XDG configs
├── defaults.rs    # Embedded default files and ConfigBootstrapper
├── models/        # Serde models (settings, menu, profile, icons, style, bundle, script, snapshot)
├── repos/         # SettingsRepo, ProfileRepo, TemplatesRepo, IconPackRepo
├── services/      # SnapshotStore and StylesManager
└── utils/         # Slug sanitization and TOML read/write helpers
```

## Testing

```bash
cargo test -p genubar-config
```
