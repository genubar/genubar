# genubar-gtk4-hook (libgenubar_gtk4_hook.so)

An `LD_PRELOAD` shared library written in Rust (`cdylib` + `rlib` for unit testing) that extracts in-window hamburger/popover menus (`GtkMenuButton`) from modern **GTK4 / GTK3 / Libadwaita** applications and exports them to the **Genubar** global menubar daemon over D-Bus.

## Why This Hook Exists

Modern GTK4 / Libadwaita applications rarely use a traditional `GtkPopoverMenuBar`. Instead, they embed one or more `GtkMenuButton` hamburger buttons inside `AdwHeaderBar` and bind actions to internal widget-level `GtkActionMuxer` scopes (such as `page.*`, `view.*`, `slot.*`, or `editor.*`) that are invisible to external D-Bus clients.

`libgenubar_gtk4_hook.so` bridges this gap without requiring source-code modifications to GTK applications:

1. **Zero Compile-Time GTK Linking:** Resolves GTK / GLib / GIO / Adwaita / WebKit symbols dynamically via `dlsym(RTLD_DEFAULT, ...)` at runtime so it never pollutes non-GTK processes with `DT_NEEDED` dependencies.
2. **Child-Process Safe:** Clears `LD_PRELOAD` inside `g_application_run` before starting the main loop so child processes (shells, compilers, browsers) spawned by the app run cleanly.
3. **Widget-Tree Menu Extraction & Live Signal Tracking:** Walks realized `GtkWindow` widget trees on `map`, collects every `GtkMenuButton`'s `GMenuModel`, tracks `GtkWidget::insert_action_group` and `GActionGroup` change signals (`action-added`, `action-removed`, `action-enabled-changed`, `action-state-changed`), infers column titles (`File`, `View`, `Options`), optionally hides original hamburger buttons (`GENUBAR_GTK4_HIDE_BUTTONS=1`), and cleans up destroyed widgets automatically via `g_object_weak_ref`.
4. **In-Process Action & Zoom Bridge (`org.genubar.Gtk4Bridge`):** Exports `/org/genubar/gtk4_bridge` to dispatch widget-scoped actions, report structured `GetActionStates` tuples (`a(sbss)`), switch Libadwaita color schemes (`System` / `Light` / `Dark`), and handle universal Zoom controls (`Zoom In`, `Zoom Out`, `Reset Zoom` with live percentage tracking).

## Module Structure

```text
src/
├── lib.rs                 # Crate root and re-exports of exported C hook symbols
├── hooks.rs               # Exported C interceptors (`g_application_run`, `g_module_symbol`, GTK hooks)
├── gi_hook.rs             # GModule / PyGObject / GJS symbol redirection and real symbol cache
├── ffi.rs                 # Opaque C types, GDBus VTable, GVariant builders, and dynamic dlsym wrappers
├── instance_guard.rs      # Single-instance guard terminating stale unhooked instances
├── logger.rs              # Diagnostic hook logging (`GENUBAR_GTK4_DEBUG=1`)
├── state/
│   ├── mod.rs             # Thread-safe HookState facade over window, export, and widget state
│   ├── window_context.rs  # Active window/app pointers, known windows, and per-window zoom state
│   ├── export_registry.rs # GDBus export IDs, GMenuModel lifecycle, and export signature cache
│   └── widget_tracking.rs # Intercepted GActionGroups, popovers, hidden buttons, and weak-ref eviction
├── scanner.rs             # Window scanning, GMenu memory management, and debounced rescan timers
├── widget_tree.rs         # Live GTK widget traversal (GtkStack / AdwViewStack aware)
├── button_heuristics.rs   # Pure heuristics for menu column titles and header button hiding
├── menu_extractor.rs      # MenuExtractor: builds the aggregated master GMenu per window
├── action_states.rs       # Collects structured (action, enabled, kind, value) action states
├── debug_dump.rs          # Diagnostic DumpIntrospection report builder
├── variant_utils.rs       # Safe GVariant unboxing and g_action_parse_detailed_name RAII wrappers
├── action_dispatcher.rs   # Dispatches actions across live GActionGroups, widget muxers, and roots
├── bridge.rs              # org.genubar.Gtk4Bridge D-Bus method handler
├── theme_zoom.rs          # Adwaita AdwStyleManager theme switching and WebKit/GTK zoom control
└── exporter.rs            # GDBus exporter (/org/genubar/gtk4_*) and WindowTracker notifier
```

## Exported D-Bus Objects

When a GTK window is mapped, the hook exports the following paths on the application's session D-Bus connection and calls `org.genubar.WindowTracker.RegisterGtk4Window`:

| Object Path | Interface | Purpose |
| --- | --- | --- |
| `/org/genubar/gtk4_menu` | `org.gtk.Menus` | Aggregated `GMenuModel` containing all extracted `GtkMenuButton` models |
| `/org/genubar/gtk4_app` | `org.gtk.Actions` | Application-level `GActionGroup` |
| `/org/genubar/gtk4_win` | `org.gtk.Actions` | Window-level `GActionGroup` |
| `/org/genubar/gtk4_bridge` | `org.genubar.Gtk4Bridge` | In-process dispatcher for `Activate`, `GetActionStates` (`a(sbss)`), `GetZoomPercent`, `Refresh`, `DumpIntrospection` |

## Building & Testing

```bash
# Run unit tests
cargo test -p genubar-gtk4-hook

# Build the shared library (.so)
cargo build -p genubar-gtk4-hook --release

# Launch any GTK4 / Libadwaita application with the hook preloaded
GENUBAR_GTK4_HIDE_BUTTONS=1 LD_PRELOAD=$PWD/target/release/libgenubar_gtk4_hook.so gnome-text-editor
```
