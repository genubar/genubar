# genubar-dbus

Async D-Bus protocol clients and daemon D-Bus server adapters connecting GNOME Shell, applications, and the accessibility bus.

## Protocol Clients

- **Canonical DBusMenu (`protocols/canonical/`):** Implements `com.canonical.dbusmenu` (`GetLayout`, `Event`, `AboutToShow`), supporting two-pass preloading (`CanonicalFetchOptions`) and lazy child submenu fetching for Qt, Electron, Chromium, and JetBrains/JVM applications.
- **GTK & Hooked GTK4 (`protocols/gtk/`):** Reads `org.gtk.Menus` + `org.gtk.Actions`, queries live widget states and introspection from `org.genubar.Gtk4Bridge`, dispatches parameterized/stateful actions, and synthesizes synthetic custom-widget items (`custom_widgets.rs`, `hook_bridge.rs`).
- **AT-SPI2 Accessibility (`protocols/atspi/`):** Connects to the accessibility bus (`a11y_bus.rs`), discovers menu bars by PID (`discovery.rs`), safely walks trees within depth/node budgets (`tree_walker.rs`), optionally probes submenus (`menu_opener.rs`), caches node paths (`cache.rs`), and activates items (`state_probe.rs`, `client.rs`).
- **Discovery & Helpers (`introspect.rs`, `utils/shortcuts.rs`, `utils/variant.rs`, `unified.rs`):** D-Bus XML introspection (`DbusIntrospector`), GTK/AT-SPI accelerator parsing, GVariant decoding, and `UnifiedMenuService` facade.

## D-Bus Server Interfaces

`server/` exposes two thin zbus adapters delegating via async backend ports (`TrackerBackend`, `RegistrarBackend`, `MenuSignalSink`):

- **WindowTrackerService (`server/tracker.rs`, `org.genubar.WindowTracker`):** Handles `NotifyFocusChanged`, `GetActiveMenu`, `FetchSubmenu`, `ActivateItem`, `RegisterGtk4Window`, `RefreshCanonicalWindow`, `ReloadConfig`, `ClearCaches`, `CaptureActiveSnapshot`, `CreateProfileFromSnapshot`, `GetDaemonInfo`, and emits `ActiveMenuChanged`.
- **AppMenuRegistrar (`server/registrar.rs`, `com.canonical.AppMenu.Registrar`):** Tracks Canonical `RegisterWindow` / `UnregisterWindow` / `GetMenuForWindow` registrations by XWayland window ID and triggers asynchronous panel updates when an app registers after receiving focus.

## Testing

```bash
cargo test -p genubar-dbus
```
