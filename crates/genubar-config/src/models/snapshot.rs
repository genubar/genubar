use genubar_core::{MenuNode, MenuSourceTier};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuSnapshot {
    pub app_id: String,
    pub window_title: String,
    pub captured_from_tier: MenuSourceTier,
    pub root: MenuNode,
}