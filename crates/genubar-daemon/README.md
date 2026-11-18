# genubar-daemon

Runtime background service that coordinates focused window resolution, tiered menu extraction, L1 disk caching, lazy submenu expansion, and action dispatch over session D-Bus.

## How It Works

When `genubar-daemon` starts (`run_daemon` / `run_daemon_with_service` in `src/lib.rs`), it registers both `com.canonical.AppMenu.Registrar` and `org.genubar.WindowTracker` on the session D-Bus and runs until the session bus closes (`conn.closed()`) or `SIGTERM` / `SIGINT` is received.

## Architecture

```text
src/
├── lib.rs              # Service registration and D-Bus / signal shutdown loop
├── main.rs             # CLI entrypoint (`genubar-daemon daemon`)
├── coordinator.rs      # FocusCoordinator: handles focus events, config reload, and active state
├── resolver.rs         # FocusMenuResolver: matches focus target against live tiers & fallbacks
├── tier_acquirer.rs    # Queries GtkNative, Gtk4Hook, CanonicalDbus, and Atspi with timeouts
├── fallback.rs         # Desktop Idle menu & Default-Fallback profile/app builder
├── submenu_service.rs  # On-demand FetchSubmenu handler + L1/Canonical/AT-SPI cache sync
├── action_dispatch.rs  # Routes item activations across GTK, DBusMenu, AT-SPI, and custom actions
├── executor.rs         # Executes shell commands, URIs, D-Bus calls, and system actions
├── models.rs           # Wire payload (`MenuPayload`, `UiRenderConfig`) serialized to extension
└── state/
    ├── mod.rs              # DaemonCaches state bundle
    ├── window_registry.rs  # Maps window XID/PID/GTK bus names to targets
    ├── canonical_cache.rs  # In-memory recent DBusMenu roots
    └── l1_cache.rs         # Persistent L1 disk cache under `~/.cache/genubar/`
```

## Resolution Order on Focus Change

1. Empty desktop or unowned focus resolves through `DesktopIdle` (`desktop_menu.toml`).
2. Focused windows check native GTK (`org.gtk.Menus`), hooked GTK4 (`org.genubar.Gtk4Bridge`), Canonical AppMenu Registrar, and AT-SPI2 in configured tier order (`tier_acquirer.rs`).
3. Raw trees pass through `MenuPipeline::finalize_menu_pipeline` (profiles, Python transformers, `app_menu.toml`, sorting, overflow, icons) and are emitted as JSON via `ActiveMenuChanged` and cached to `MenuPayload`.

## Running & Testing

```bash
# Run foreground daemon with debug logs
RUST_LOG=debug cargo run -p genubar-daemon

# Run unit tests
cargo test -p genubar-daemon
```
