use std::path::Path;
use std::process::Command;

/// Opens a URL or local path in the user's default browser or file manager.
pub fn open_external(target: &str) {
    let _ = Command::new("xdg-open").arg(target).spawn();
}

pub fn open_directory(path: &Path) {
    let _ = Command::new("xdg-open").arg(path).spawn();
}