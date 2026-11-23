# genubar-cli

Unified command-line diagnostics, live protocol inspector, patch/bundle manager, and foreground daemon runner (`genubar-cli`).

## Subcommands

Defined in `src/cli.rs` and implemented across `src/commands/`:

- **Service & Live Control (`commands/daemon.rs`):**
  - `daemon` — runs `AppMenu.Registrar` and `org.genubar.WindowTracker` in the foreground.
  - `reload-config` — instructs the active daemon to reload `~/.config/genubar/`.
  - `capture-snapshot` — captures the active application's menu tree to `~/.config/genubar/snapshots/`.
  - `profile-from-snapshot --snapshot <FILE> [--sync-new]` — generates a TOML profile from a captured snapshot.
  - `simulate-focus [--pid <PID>] [--xid <XID>] [--title <TITLE>]` — triggers a synthetic focus event.
- **D-Bus & Accessibility Inspection (`commands/inspect.rs`, `printer.rs`):**
  - `discover --dest <BUS_NAME>` — discovers Canonical and GTK menu/action object paths on a D-Bus peer.
  - `inspect --dest <BUS_NAME> [--path /MenuBar/2]` — dumps a Canonical DBusMenu tree.
  - `inspect-gtk --dest <BUS_NAME> --menubar-path <PATH> [--app-menu-path ...] [--app-path ...] [--win-path ...]` — dumps a GTK `GMenuModel` tree.
  - `inspect-atspi --pid <PID>` — extracts menus via AT-SPI2 by process ID.
  - `dump-atspi --pid <PID> [--depth 6] [--include-doc]` — dumps raw AT-SPI2 accessible hierarchies for debugging.
  - `dump-gtk4 --pid <PID>` — queries `org.genubar.Gtk4Bridge.DumpIntrospection` on a hooked GTK4 app.
  - `click --dest <BUS_NAME> --id <ID> [--path /MenuBar/2]` — activates a specific menu item ID over Canonical DBusMenu.
  - `click-gtk --dest <BUS_NAME> --menubar-path <PATH> --id <ID> [--app-path ...] [--win-path ...]` — activates a specific menu item ID over GTK actions.
- **Patches & Bundles (`commands/packages.rs`):**
  - `patches`, `apply-patch --id <ID> [--skip-unsupported]`, `revert-patch --id <ID> [--force]`
  - `bundles`, `apply-bundle --id <ID>`, `revert-bundle --id <ID>`

## Examples

```bash
genubar-cli reload-config
genubar-cli inspect-atspi --pid 12345
genubar-cli patches
genubar-cli apply-bundle --id gnome_core_suite
```
