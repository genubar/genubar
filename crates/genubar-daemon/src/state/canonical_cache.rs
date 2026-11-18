use genubar_core::MenuNode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type SubmenuCacheMap = HashMap<(u32, String), Vec<MenuNode>>;

/// In-memory RAM cache for Canonical DBusMenu roots and lazily loaded submenus.
#[derive(Default, Clone)]
pub struct CanonicalMenuCache {
    last_good_canonical: Arc<RwLock<HashMap<u32, MenuNode>>>,
    submenus_by_pid: Arc<RwLock<SubmenuCacheMap>>,
}

impl CanonicalMenuCache {
    pub async fn cache_menu(&self, pid: u32, menu: MenuNode) {
        if pid != 0 {
            self.last_good_canonical.write().await.insert(pid, menu);
        }
    }

    pub async fn get_cached_menu(&self, pid: u32) -> Option<MenuNode> {
        if pid == 0 {
            return None;
        }
        self.last_good_canonical.read().await.get(&pid).cloned()
    }

    pub async fn cache_submenu(&self, pid: u32, col_label: &str, children: Vec<MenuNode>) {
        if pid == 0 || col_label.is_empty() || children.is_empty() {
            return;
        }
        self.submenus_by_pid
            .write()
            .await
            .insert((pid, col_label.to_lowercase()), children);
    }

    pub async fn stitch_submenus(&self, pid: u32, root: &mut MenuNode) {
        if pid == 0 {
            return;
        }
        let guard = self.submenus_by_pid.read().await;
        for col in &mut root.children {
            if col.children.is_empty() {
                let key = (pid, col.clean_label().to_lowercase());
                if let Some(cached_children) = guard.get(&key) {
                    col.children = cached_children.clone();
                }
            }
        }
    }

    pub async fn clear_all(&self) {
        self.last_good_canonical.write().await.clear();
        self.submenus_by_pid.write().await.clear();
    }
}
