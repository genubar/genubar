# genubar-platform

Host operating system, XDG path resolution, process inspection, stylesheet synchronization, and GNOME Shell extension deployment layer for Genubar.

## Responsibility

`genubar-platform` centralizes all interactions with the local Linux desktop environment so higher-level crates (`genubar-config`, `genubar-patcher`, `genubar-packages`, `genubar-ui`, `genubar-installer`) never hardcode OS paths or duplicate shell/proc probing:

- **XDG & App Directories (`paths.rs`):** `home_dir`, `expand_tilde`, `resolve_user_path`, `config_dir`, `cache_dir`, `data_dir`, `bin_dir`, `lib_dir`, `extensions_dir`, `systemd_user_dir`, `user_applications_dir`, `user_dbus_services_dir`, `desktop_search_dirs`, `flatpak_app_dirs`.
- **Filesystem Utilities (`fs_utils.rs`):** Atomic text/bytes file writes, directory creation, symlink-or-copy replacement (`replace_with_atomic_copy_or_symlink`), and safe cleanup helpers.
- **Process & Hook Inspection (`proc_utils.rs`, `hook_env.rs`):** Reads `/proc/<pid>/maps` (`is_pid_gtk4_hooked`) and `/proc/<pid>/stat` (`get_parent_pid`), locates and atomically synchronizes `~/.local/lib/genubar/active_gtk4_hook.so` (`locate_or_sync_gtk4_hook_so`), and updates the D-Bus/systemd session activation environment via `dbus-update-activation-environment` (`Gtk4EnvSynchronizer`).
- **Desktop & Flatpak Discovery (`desktop_probe.rs`, `os_info.rs`):** Resolves `.desktop` files, D-Bus `.service` `Exec=` targets, `PATH` binaries, Flatpak installations, and OS display names from `/etc/os-release` (`detect_os_name`).
- **CSS Metrics Sync (`css_sync.rs`):** Injects panel/column/row pixel metrics into the active CSS preset and synchronizes it to the GNOME Shell extension stylesheet via copy, symlink, or direct mode (`CssSynchronizer`).
- **Extension Deployment (`extension_installer.rs`, `compat_probe.rs`):** Embeds the full `genubar-gnome-extension` file set (`EMBEDDED_EXT_FILES`), verifies `metadata.json` compatibility (`inspect_installed_extension`), installs/updates `genubar@github.com`, and cleans up stale files.
- **Safe Script Subprocesses & Header Parsing (`process_timeout.rs`, `header_parser.rs`):** Runs subprocesses with STDIN JSON payloads under strict timeouts (`run_command_with_stdin_timeout`) and parses `# ==GenubarScript==` / `/* ==GenubarStyle== */` metadata headers (`parse_metadata_block`).

## Testing

```bash
cargo test -p genubar-platform
```
