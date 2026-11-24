# genubar-patcher

Declarative application patch engine with pre-flight condition checks, multi-format system appliers, conflict analysis, and a transactional rollback ledger.

## Why This Crate Exists

Many Linux applications (GTK4, Electron, Qt, JetBrains IDEs, Flatpak packages) disable native menubar exports by default, activate via `DBusActivatable` without reading wrapper flags, or require environment variables (`LD_PRELOAD`, `GTK_MODULES`, `XDG_CURRENT_DESKTOP`). `genubar-patcher` automates applying and cleanly reverting these changes.

## How It Works

- **Patch Catalog (`repository.rs`, `defaults/builtin_patches.toml`):** Combines built-in patch manifests with user `.toml` manifests in `~/.config/genubar/patches/`. Supports forward-compatible deserialization (`Unsupported` steps and `Unknown` conditions/kinds from newer releases).
- **System Detection & Conditions (`detector.rs`, `condition.rs`):** Evaluates app detection rules (`.desktop`, `PATH` binaries, Flatpaks) and pre-flight `Check` steps (`FileExists`, `DirExists`, `BinaryExists`, `DesktopFileExists`, `FlatpakAppInstalled`, `EnvEquals`, etc.) with configurable `ConditionFailAction` policies.
- **Appliers (`applier/*.rs`):**
  - `desktop_override.rs`: Creates/updates user `~/.local/share/applications/*.desktop` overrides (injecting env/args, optionally forcing `DBusActivatable=false`, and resolving `gapplication launch` service executables) and pairs with `manager/process_sync.rs` to create user D-Bus overrides in `~/.local/share/dbus-1/services/<app_id>.service` (injecting env, stripping `SystemdService=`, refreshing the desktop database, and calling `org.freedesktop.DBus.ReloadConfig`).
  - `cli_wrapper.rs`: Generates wrapper scripts in `~/.local/bin/` for terminal-launched binaries.
  - `flatpak.rs`: Configures Flatpak overrides (`--talk-name`, environment injection, read-only `~/.local/lib/genubar:ro` access for `active_gtk4_hook.so`).
  - `gsettings.rs`, `json_set.rs`, `text_line.rs`, `firefox_pref.rs`, `python_script.rs`: Handles GSettings keys, JSON dot-paths, appended config lines, Firefox `user.js` preferences, and sandboxed patch scripts.
- **Transactional Ledger (`ledger.rs`):** Records every touched resource, previous state/backup, and patch ownership (`owners`) inside `~/.local/share/genubar/patch_ledger.json` so shared resources are only restored when the last owning patch is reverted.
- **Process Synchronization (`manager/process_sync.rs`):** Prompts/terminates affected background instances when required after applying/reverting hooks.

## Testing

```bash
cargo test -p genubar-patcher
```
