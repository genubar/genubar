use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "genubar-cli",
    version,
    about = "Unified CLI inspector and daemon for Canonical & GTK Menus"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run both AppMenu.Registrar and org.genubar.WindowTracker services
    Daemon,

    /// Discover Canonical and GTK menu/action object paths on a D-Bus peer
    Discover {
        #[arg(short, long)]
        dest: String,
    },

    /// Inspect a Canonical DBusMenu tree
    Inspect {
        #[arg(short, long)]
        dest: String,
        #[arg(short, long, default_value = "/MenuBar/2")]
        path: String,
    },

    /// Inspect a GTK (org.gtk.Menus + org.gtk.Actions) menu tree
    InspectGtk {
        #[arg(short, long)]
        dest: String,
        #[arg(short, long)]
        menubar_path: String,
        #[arg(long)]
        app_menu_path: Option<String>,
        #[arg(long)]
        app_path: Option<String>,
        #[arg(long)]
        win_path: Option<String>,
    },

    /// Inspect an application's menu via the AT-SPI2 Accessibility Bus by OS PID
    InspectAtspi {
        #[arg(short, long)]
        pid: u32,
    },

    /// Dump the raw AT-SPI2 accessibility widget tree of a process for profile diagnostics
    DumpAtspi {
        #[arg(short, long)]
        pid: u32,
        #[arg(short, long, default_value_t = 6)]
        depth: usize,
        /// Traverse inside "document web" / CEF containers (required for Spotify / Electron)
        #[arg(long, default_value_t = false)]
        include_doc: bool,
    },

    /// Click an item in a Canonical DBusMenu
    Click {
        #[arg(short, long)]
        dest: String,
        #[arg(short, long, default_value = "/MenuBar/2")]
        path: String,
        #[arg(short, long)]
        id: i32,
    },

    /// Click an item in a GTK Menu
    ClickGtk {
        #[arg(short, long)]
        dest: String,
        #[arg(short, long)]
        menubar_path: String,
        #[arg(long)]
        app_path: Option<String>,
        #[arg(long)]
        win_path: Option<String>,
        #[arg(short, long)]
        id: i32,
    },

    /// Simulate a window focus event sent to the running daemon
    SimulateFocus {
        #[arg(long, default_value_t = 0)]
        pid: u32,
        #[arg(long, default_value_t = 0)]
        xid: u32,
        #[arg(long, default_value = "Test Window")]
        title: String,
    },

    /// Reload all configuration files in ~/.config/genubar/ without restarting the daemon
    ReloadConfig,

    /// Capture the currently active application's menu tree to ~/.config/genubar/snapshots/
    CaptureSnapshot,

    /// Generate a declarative TOML profile in ~/.config/genubar/profiles/ from a captured snapshot JSON
    ProfileFromSnapshot {
        /// Snapshot filename (e.g. "firefox.json") or full path
        #[arg(short, long)]
        snapshot: String,
        /// Dynamically load new application items not present in the snapshot
        #[arg(long, default_value_t = false)]
        sync_new: bool,
    },

    /// List all patches (Applied, Recommended for this machine, and All)
    Patches,

    /// Apply a patch by ID (with automatic backup into patch_ledger.json)
    ApplyPatch {
        #[arg(short, long)]
        id: String,
        /// Skip unknown step types from newer Genubar versions
        #[arg(long, default_value_t = false)]
        skip_unsupported: bool,
    },
    /// Revert an applied patch and restore original system settings
    RevertPatch {
        #[arg(short, long)]
        id: String,
        /// Force unsafe best-effort cleanup even if patch_ledger.json is missing/corrupted
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Dump all live GActionGroups, actions, and widget hierarchy from a hooked GTK4 process by PID
    DumpGtk4 {
        #[arg(short, long)]
        pid: u32,
    },

    /// List all installed bundles and their component status
    Bundles,
    /// Apply all patches and enable all profiles belonging to a bundle
    ApplyBundle {
        #[arg(short, long)]
        id: String,
    },
    /// Revert all patches belonging to a bundle
    RevertBundle {
        #[arg(short, long)]
        id: String,
    },
}
