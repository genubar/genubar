//! Self-contained installer (`genubar-installer`) for Genubar binaries, the GTK4 hook, GNOME extension, and systemd user unit.

mod artifacts;
mod error;
mod operations;
mod ui;

use crate::operations::{install_extension_only, perform_install_or_update, perform_uninstall};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--install" || a == "--update") {
        return match perform_install_or_update() {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.iter().any(|a| a == "--install-extension") {
        return match install_extension_only() {
            Ok(ver) => {
                println!(
                    "Installed GNOME Shell extension {} v{ver}",
                    genubar_core::EXTENSION_UUID
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.iter().any(|a| a == "--uninstall") {
        let purge = args.iter().any(|a| a == "--purge");
        return match perform_uninstall(purge) {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    ui::run_gui();
    ExitCode::SUCCESS
}
