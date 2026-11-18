use async_trait::async_trait;
use crate::errors::MenuError;
use crate::node::{MenuNode, MenuTarget};


#[async_trait]
pub trait MenuProvider: Send + Sync {
    /// Fetches the complete menu hierarchy for a given target
    async fn fetch_menu(&self, target: &MenuTarget) -> Result<MenuNode, MenuError>;

    /// Fetches and expands a single top-level or nested submenu on demand (used by lazy providers such as JetBrains Swing).
    async fn fetch_submenu(
        &self,
        _target: &MenuTarget,
        _submenu_id: i32,
    ) -> Result<MenuNode, MenuError> {
        Err(MenuError::NotFound(
            "On-demand submenu fetch not supported by this provider".into(),
        ))
    }

    /// Triggers the click/activation action on a specific menu item ID
    async fn activate_item(&self, target: &MenuTarget, item_id: i32) -> Result<(), MenuError>;
}