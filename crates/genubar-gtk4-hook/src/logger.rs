//! Diagnostic logger for `libgenubar_gtk4_hook.so`.
//!
//! Writes structured trace events to `~/.cache/genubar/gtk4_hook.log`
//! (and to `stderr` when `GENUBAR_GTK4_DEBUG=1` is set).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn log_message(msg: &str) {
    let pid = unsafe { libc::getpid() };
    let comm = fs::read_to_string("/proc/self/comm")
        .unwrap_or_default()
        .trim()
        .to_string();
    let formatted = if comm.is_empty() {
        format!("[GTK4-Hook PID={pid}] {msg}\n")
    } else {
        format!("[GTK4-Hook PID={pid} ({comm})] {msg}\n")
    };

    if env::var("GENUBAR_GTK4_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        eprint!("{formatted}");
    }

    if let Some(log_path) = log_file_path() {
        if let Some(parent) = log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = file.write_all(formatted.as_bytes());
        }
    }
}

fn log_file_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".cache/genubar/gtk4_hook.log"))
}

#[macro_export]
macro_rules! hook_log {
    ($($arg:tt)*) => {
        $crate::logger::log_message(&format!($($arg)*));
    };
}
