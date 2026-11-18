use crate::error::UiError;
use crate::services::daemon_ipc::DaemonIpcService;
use genubar_platform::fs_utils::remove_file_if_exists;

pub struct CacheService;

impl CacheService {
    pub fn clear_l1_cache() -> Result<(), UiError> {
        let _ = DaemonIpcService::clear_caches(true, false);
        Self::remove_cache_file("l1_roots.json")
    }

    pub fn clear_l2_atspi_cache() -> Result<(), UiError> {
        let _ = DaemonIpcService::clear_caches(false, true);
        Self::remove_cache_file("atspi_menus.json")
    }

    fn remove_cache_file(name: &str) -> Result<(), UiError> {
        let path = genubar_platform::cache_dir().join(name);
        remove_file_if_exists(&path).map_err(UiError::Domain)
    }
}
