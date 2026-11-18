//! Unified CLI diagnostic tool, patch/bundle manager, and foreground daemon runner (`genubar-cli`).

mod cli;
mod commands;
mod printer;

use clap::Parser;
use cli::{Cli, Commands};
use genubar_dbus::unified::UnifiedMenuService;

fn requires_dbus_session(command: &Commands) -> bool {
    !matches!(
        command,
        Commands::Patches
            | Commands::ApplyPatch { .. }
            | Commands::RevertPatch { .. }
            | Commands::Bundles
            | Commands::ApplyBundle { .. }
            | Commands::RevertBundle { .. }
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Patches => commands::packages::list_patches()?,
        Commands::ApplyPatch {
            id,
            skip_unsupported,
        } => commands::packages::apply_patch(id, skip_unsupported)?,
        Commands::RevertPatch { id, force } => commands::packages::revert_patch(id, force)?,
        Commands::Bundles => commands::packages::list_bundles()?,
        Commands::ApplyBundle { id } => commands::packages::set_bundle_enabled(id, true).await?,
        Commands::RevertBundle { id } => commands::packages::set_bundle_enabled(id, false).await?,
        dbus_cmd => run_dbus_command(dbus_cmd).await?,
    }

    Ok(())
}

async fn run_dbus_command(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    debug_assert!(requires_dbus_session(&command));
    let service = UnifiedMenuService::new().await?;

    match command {
        Commands::Daemon => commands::daemon::run_daemon(&service).await?,
        Commands::Discover { dest } => commands::inspect::discover(&service, dest).await?,
        Commands::Inspect { dest, path } => {
            commands::inspect::inspect_canonical(&service, dest, path).await?
        }
        Commands::InspectGtk {
            dest,
            menubar_path,
            app_menu_path,
            app_path,
            win_path,
        } => {
            commands::inspect::inspect_gtk(
                &service,
                dest,
                menubar_path,
                app_menu_path,
                app_path,
                win_path,
            )
            .await?
        }
        Commands::InspectAtspi { pid } => {
            commands::inspect::inspect_atspi(&service, pid).await?
        }
        Commands::DumpAtspi {
            pid,
            depth,
            include_doc,
        } => commands::inspect::dump_atspi(&service, pid, depth, include_doc).await?,
        Commands::Click { dest, path, id } => {
            commands::inspect::click_canonical(&service, dest, path, id).await?
        }
        Commands::ClickGtk {
            dest,
            menubar_path,
            app_path,
            win_path,
            id,
        } => {
            commands::inspect::click_gtk(&service, dest, menubar_path, app_path, win_path, id)
                .await?
        }
        Commands::SimulateFocus { pid, xid, title } => {
            commands::packages::simulate_focus(&service, pid, xid, title).await?
        }
        Commands::ReloadConfig => commands::packages::reload_config(&service).await?,
        Commands::CaptureSnapshot => commands::packages::capture_snapshot(&service).await?,
        Commands::ProfileFromSnapshot { snapshot, sync_new } => {
            commands::packages::profile_from_snapshot(&service, snapshot, sync_new).await?
        }
        Commands::DumpGtk4 { pid } => commands::inspect::dump_gtk4(&service, pid).await?,
        Commands::Patches
        | Commands::ApplyPatch { .. }
        | Commands::RevertPatch { .. }
        | Commands::Bundles
        | Commands::ApplyBundle { .. }
        | Commands::RevertBundle { .. } => unreachable!("local command handled before D-Bus init"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_commands_do_not_require_dbus_session() {
        assert!(!requires_dbus_session(&Commands::Patches));
        assert!(!requires_dbus_session(&Commands::Bundles));
        assert!(!requires_dbus_session(&Commands::ApplyPatch {
            id: String::from("demo"),
            skip_unsupported: false,
        }));
        assert!(!requires_dbus_session(&Commands::RevertPatch {
            id: String::from("demo"),
            force: false,
        }));
        assert!(!requires_dbus_session(&Commands::ApplyBundle {
            id: String::from("demo"),
        }));
        assert!(requires_dbus_session(&Commands::Daemon));
        assert!(requires_dbus_session(&Commands::ReloadConfig));
        assert!(requires_dbus_session(&Commands::InspectAtspi { pid: 1 }));
    }
}
