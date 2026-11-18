use genubar_dbus::unified::UnifiedMenuService;
use std::error::Error;

pub async fn run_daemon(service: &UnifiedMenuService) -> Result<(), Box<dyn Error>> {
    genubar_daemon::run_daemon_with_service(service).await
}
