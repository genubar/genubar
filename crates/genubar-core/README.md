# genubar-core

Pure domain models, menu tree primitives, compatibility validators, and provider abstractions shared across Genubar's configuration, pipeline, D-Bus, and UI layers.

## Responsibility

`genubar-core` isolates core business concepts from OS I/O, D-Bus transports, and GTK4 UI toolkits:

- **Menu Tree Model (`node.rs`):** `MenuNode`, `MenuItemType` (`Standard`, `Separator`, `SearchBar`, `WindowList`, `Check`, `Radio`, `Submenu`), `MenuSourceTier`, `MenuTarget`, `ActiveWindowInfo`, `AtspiSearchConfig`, mnemonic label normalization (`clean_label`), and case-insensitive glob matching (`wildcard_match`).
- **Live Tree Helpers (`node.rs`):** Lazy child attachment (`attach_submenu_children`), ID/path toggle updates with separator-bounded radio groups (`toggle_check_or_radio_by_id`, `toggle_check_or_radio_by_path`), and origin-path indexing (`populate_origin_paths`).
- **Action Primitives (`action.rs`):** `SystemCommand` (`QuitApp`, `ReloadConfig`, `CaptureMenuSnapshot`, window actions, text case transforms) and `CustomAction` (`Command`, `Shell`, `OpenUri`, `System`, `CopyText`, `InsertText`, `KeyShortcut`, `DbusCall`, `GtkAction`).
- **Provider Port (`traits.rs`):** Async `MenuProvider` trait (`fetch_menu`, `fetch_submenu`, `activate_item`) implemented by D-Bus protocol backends.
- **Version & Extension Compatibility (`compat.rs`):** SemVer comparison (`parse_semver_triple`, `is_version_compatible`), `check_extension_metadata_json`, and `ExtensionCompatReport`.
- **Errors & Package Taxonomy (`errors.rs`, `package.rs`):** `MenuError` and forward-compatible `PackageKind`.

## Module Map

```text
src/
├── lib.rs       # Re-exports public domain types, traits, and version constants
├── node.rs      # MenuNode tree, MenuItemType, clean_label, wildcard_match
├── action.rs    # SystemCommand and CustomAction parsers/models
├── compat.rs    # Daemon <-> GNOME extension compatibility checks
├── errors.rs    # Domain MenuError enum
├── package.rs   # Forward-compatible PackageKind taxonomy
└── traits.rs    # Async MenuProvider trait
```

## Testing

```bash
cargo test -p genubar-core
```
