# genubar-protocol

Zero-dependency shared IPC contracts, D-Bus service constants, cross-process custom action identifiers, and GNOME Shell extension metadata constants used across the entire Genubar workspace.

## Why This Crate Exists

Both the main async Tokio workspace (`genubar-daemon`, `genubar-dbus`, `genubar-ui`, `genubar-installer`) and the lightweight `LD_PRELOAD` shared library (`genubar-gtk4-hook`) must agree on exact D-Bus bus names, object paths, `CStr` literals, action prefixes, and protected process guard lists without pulling heavyweight async dependencies into hooked GTK processes.

`genubar-protocol` keeps zero external crate dependencies (`std` only) so every binary and `.so` hook can depend on it safely.

## What It Defines

- **D-Bus Contracts:**
  - `TRACKER_BUS_NAME` / `TRACKER_OBJECT_PATH` / `TRACKER_INTERFACE` (`org.genubar.WindowTracker`) and their `*\_C` `CStr` counterparts.
  - `REGISTRAR_BUS_NAME` / `REGISTRAR_OBJECT_PATH` (`com.canonical.AppMenu.Registrar`).
  - `GTK4_BRIDGE_INTERFACE` / `GTK4_BRIDGE_PATH` / `GTK4_MENU_PATH` / `GTK4_APP_PATH` / `GTK4_WIN_PATH` (`/org/genubar/gtk4_*`) and their `*\_C` counterparts.
- **Cross-Process Action Contracts:** `ACTION_THEME_SYSTEM`, `ACTION_THEME_LIGHT`, `ACTION_THEME_DARK`, `ACTION_ZOOM_IN`, `ACTION_ZOOM_OUT`, `ACTION_ZOOM_RESET`, and `HIDDEN_BUTTONS_ENV` (`GENUBAR_GTK4_HIDE_BUTTONS`).
- **GNOME Shell Extension Metadata:** `EXTENSION_UUID` (`genubar@github.com`) and `EXTENSION_FILES` (canonical manifest of all 24 extension files synchronized with `genubar-platform`).
- **Hook Library & Safety Guards:** `GTK4_HOOK_ACTIVE_SO_NAME`, `GTK4_HOOK_CANONICAL_SO_NAME`, `GTK4_HOOK_MAP_MARKERS`, `PROTECTED_SHELL_BINARIES`, and `PROTECTED_HOOK_GUARD_BINARIES`.

## Testing

```bash
cargo test -p genubar-protocol
```
