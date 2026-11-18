use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "genubar-daemon",
    version,
    about = "Background D-Bus daemon serving global menubar menus for GNOME Shell"
)]
struct DaemonCli {
    #[command(subcommand)]
    command: Option<DaemonCommand>,
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Run the daemon (default when launched without arguments)
    Daemon,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let _cli = DaemonCli::parse();
    genubar_daemon::run_daemon().await
}
