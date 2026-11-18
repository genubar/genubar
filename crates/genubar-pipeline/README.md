# genubar-pipeline

Multi-stage menu transformation pipeline that converts raw backend trees (or static/profile definitions) into finalized global menubar hierarchies ready for GNOME Shell.

## Pipeline Stages

`MenuPipeline` (`src/orchestrator.rs`) transforms raw extracted menus into normalized desktop-consistent menubars through modular stages:

1. **Static & Layout Construction (`static_builder.rs`, `gtk4_layout.rs`, `gtk4_rules.rs`):** Builds static menus (`desktop_menu.toml`, fallback profiles), expands placeholders (`{os_name}`, `{app_name}`), and classifies extracted GTK4 widgets/actions into logical columns (`File`, `Edit`, `View`, `Tools`, `Help`).
2. **Profile Inheritance (`inherit.rs`):** Merges native live menu trees with declarative `AppProfile` columns (`type = "inherit"`, `inherit_others`, alias matching, `sync_new_items`, overrides).
3. **Python Menu Transformers (`python.rs`):** Executes user `# ==GenubarScript==` Python scripts under a strict timeout (`DEFAULT_SCRIPT_TIMEOUT_MS`), passing `{ "window": ..., "tier": ..., "root": ... }` via STDIN and reading the transformed tree from STDOUT.
4. **Mandatory Template Merging (`mandatory.rs`):** Applies universal `app_menu.toml` entries (application system column, standard `Edit` transforms/shortcuts, `Help` inline `SearchBar`, `WindowList` expander), preserving unexpanded lazy AT-SPI2 / DBusMenu columns until `FetchSubmenu` runs (`apply_app_menu_to_fetched_submenu`).
5. **Root Column Sorting & Overflow (`sorter.rs`, `postprocess.rs`):** Orders top-level columns per `RootSortConfig` (standard, custom order, or Python script), sanitizes duplicate/leading/trailing separators (`sanitize_separators`), optionally strips icons/search/action items, and groups excess columns into `More` (`apply_more_overflow`).
6. **Icon Resolution (`icons.rs`, `icon_matcher.rs`):** Evaluates enabled custom `IconPackManifest`s in priority order before falling back to global `icons.toml` `[[system_rules]]`.

## Testing

```bash
cargo test -p genubar-pipeline
```
