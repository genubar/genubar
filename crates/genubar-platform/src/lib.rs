//! Host/OS/XDG/filesystem/process/environment integration crate for Genubar.

pub mod compat_probe;
pub mod css_sync;
pub mod desktop_probe;
pub mod extension_installer;
pub mod fs_utils;
pub mod header_parser;
pub mod hook_env;
pub mod os_info;
pub mod paths;
pub mod proc_utils;
pub mod process_timeout;

pub use compat_probe::{
    inspect_installed_extension, query_binary_version, query_installed_daemon_version,
};
pub use css_sync::{CssMetrics, CssSyncModeKind, CssSyncOptions, CssSynchronizer};
pub use desktop_probe::{
    any_binary_in_path, any_flatpak_app, has_any_desktop_file, has_binary_in_path,
    has_flatpak_app, locate_desktop_file, parse_desktop_exec_bin, read_dbus_service_exec,
    read_desktop_exec_bin, system_dbus_service_path, user_dbus_service_path,
};
pub use extension_installer::{
    EMBEDDED_EXT_FILES, check_embedded_extension_compatibility, install_embedded_extension,
    sync_extension_gsettings, update_gsettings_extension_list, write_downloaded_extension_files,
};
pub use header_parser::parse_metadata_block;
pub use hook_env::{
    Gtk4EnvSyncOptions, Gtk4EnvSynchronizer, locate_or_sync_gtk4_hook_so, newest_gtk4_hook_so,
    resolve_installed_gtk4_hook_so,
};
pub use os_info::{detect_os_name, parse_os_name};
pub use paths::{
    APP_ICON_NAME, EMBEDDED_APP_ICON_SVG, INSTALLER_ICON_NAME, bin_dir, cache_dir, config_dir,
    data_dir, desktop_search_dirs, expand_tilde, extensions_dir, flatpak_app_dirs, home_dir,
    lib_dir, resolve_user_path, systemd_user_dir, user_applications_dir,
    user_dbus_services_dir, user_icons_scalable_apps_dir,
};
pub use proc_utils::{
    get_parent_pid, get_process_comm, is_pid_gtk4_hooked, parse_stat_ppid, pids_match,
    read_proc_cmdline, terminate_pid, terminate_unhooked_processes, try_check_pid_gtk4_hooked,
};
pub use process_timeout::{ProcessTimeoutOutcome, run_command_with_stdin_timeout};
