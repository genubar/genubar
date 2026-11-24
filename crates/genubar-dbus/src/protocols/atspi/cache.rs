use crate::models::atspi::AtspiActionRef;
use crate::protocols::atspi::parser::AtspiMenuParser;
use genubar_core::{MenuNode, clean_label};
use genubar_platform::cache_dir;
use genubar_platform::proc_utils::get_process_comm;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, RwLock};

pub type PersistentSubmenuMap = HashMap<String, Vec<MenuNode>>;

#[derive(Clone)]
pub struct AtspiSubmenuCache {
    inner: Arc<RwLock<PersistentSubmenuMap>>,
    disk_enabled: Arc<AtomicBool>,
    cache_path: PathBuf,
    flush_notify: Arc<Notify>,
    worker_started: Arc<AtomicBool>,
}

impl AtspiSubmenuCache {
    pub fn load_or_new() -> Self {
        Self::load_with_options(cache_dir().join("atspi_menus.json"), true)
    }

    pub fn load_with_options(cache_path: PathBuf, enable_disk_cache: bool) -> Self {
        let map = if enable_disk_cache {
            Self::load_from_disk(&cache_path)
        } else {
            HashMap::new()
        };
        Self {
            inner: Arc::new(RwLock::new(map)),
            disk_enabled: Arc::new(AtomicBool::new(enable_disk_cache)),
            cache_path,
            flush_notify: Arc::new(Notify::new()),
            worker_started: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_disk_cache_enabled(&self, enabled: bool) {
        self.disk_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_disk_cache_enabled(&self) -> bool {
        self.disk_enabled.load(Ordering::Relaxed)
    }

    fn load_from_disk(path: &PathBuf) -> PersistentSubmenuMap {
        if let Ok(data) = fs::read_to_string(path)
            && let Ok(map) = serde_json::from_str::<PersistentSubmenuMap>(&data)
        {
            tracing::debug!(count = map.len(), path = %path.display(), "Loaded AT-SPI2 cached submenus from disk");
            return map;
        }
        HashMap::new()
    }

    fn write_snapshot_to_disk(path: &PathBuf, map: &PersistentSubmenuMap) {
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            tracing::warn!(error = %err, path = %parent.display(), "Failed to create AT-SPI2 cache directory");
            return;
        }
        match serde_json::to_string_pretty(map) {
            Ok(json) => {
                if let Err(err) = fs::write(path, json) {
                    tracing::warn!(error = %err, path = %path.display(), "Failed to persist AT-SPI2 submenu cache");
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed to serialize AT-SPI2 submenu cache");
            }
        }
    }

    fn ensure_background_worker(&self) {
        if self
            .worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.worker_started.store(false, Ordering::Release);
            return;
        };

        let inner = self.inner.clone();
        let disk_enabled = self.disk_enabled.clone();
        let path = self.cache_path.clone();
        let notify = self.flush_notify.clone();

        handle.spawn(async move {
            loop {
                notify.notified().await;
                tokio::time::sleep(Duration::from_millis(250)).await;
                if !disk_enabled.load(Ordering::Relaxed) {
                    continue;
                }
                let snapshot = inner.read().await.clone();
                let path_c = path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    Self::write_snapshot_to_disk(&path_c, &snapshot);
                })
                .await;
            }
        });
    }

    fn schedule_persist(&self) {
        if !self.disk_enabled.load(Ordering::Relaxed) {
            return;
        }
        self.ensure_background_worker();
        self.flush_notify.notify_one();
    }

    pub async fn has_non_empty(&self, cache_key: &str) -> bool {
        self.inner
            .read()
            .await
            .get(cache_key)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    pub async fn store_submenu(
        &self,
        app_comm: &str,
        current_path: &[String],
        children: Vec<MenuNode>,
    ) {
        let cache_key = format!("{app_comm}::{}", current_path.join("::"));
        {
            let mut guard = self.inner.write().await;
            guard.insert(cache_key, children.clone());

            if current_path.len() >= 2 {
                let parent_slice = &current_path[..current_path.len() - 1];
                let parent_key = format!("{app_comm}::{}", parent_slice.join("::"));
                let target_child_name = &current_path[current_path.len() - 1];
                if let Some(parent_items) = guard.get_mut(&parent_key) {
                    for item in parent_items.iter_mut() {
                        if clean_label(&item.label) == *target_child_name {
                            item.children = children.clone();
                        }
                    }
                }
            }
        }
        self.schedule_persist();
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn restore_and_rebind(
        &self,
        app_comm: &str,
        cache_key: &str,
        next_uid: &AtomicI32,
        bus_name: &str,
        menubar_root: &str,
        current_path: &mut Vec<String>,
        action_cache: &mut HashMap<i32, AtspiActionRef>,
        paths_map: &mut HashMap<i32, Vec<String>>,
    ) -> Option<Vec<MenuNode>> {
        let guard = self.inner.read().await;
        let mut cached_children = guard.get(cache_key)?.clone();
        if cached_children.is_empty() {
            return None;
        }
        AtspiMenuParser::rebind_cached_nodes(
            app_comm,
            &guard,
            &mut cached_children,
            next_uid,
            bus_name,
            menubar_root,
            current_path,
            action_cache,
            paths_map,
        );
        Some(cached_children)
    }

    pub async fn clear_for_pid(&self, pid: u32, only_submenus: Option<&[String]>) {
        let comm = get_process_comm(pid);
        self.clear_for_comm(&comm, only_submenus).await;
    }

    pub async fn clear_for_comm(&self, comm: &str, only_submenus: Option<&[String]>) {
        let prefix = format!("{comm}::");
        {
            let mut guard = self.inner.write().await;
            guard.retain(|key, _| {
                if !key.starts_with(&prefix) {
                    return true;
                }
                if let Some(volatiles) = only_submenus {
                    !volatiles.iter().any(|v| key.contains(v))
                } else {
                    false
                }
            });
        }
        self.schedule_persist();
    }

    pub async fn update_toggle_state_by_path(
        &self,
        pid: u32,
        label_path: &[String],
        live_states: Option<&[(String, bool)]>,
    ) {
        if label_path.len() < 2 {
            return;
        }
        let comm = get_process_comm(pid);
        let app_prefix = format!("{comm}::");
        let modified = {
            let mut guard = self.inner.write().await;
            let mut dirty = false;

            for (key, nodes) in guard.iter_mut() {
                let Some(rest_key) = key.strip_prefix(&app_prefix) else {
                    continue;
                };
                let key_segments: Vec<&str> = rest_key.split("::").collect();
                if key_segments.len() >= label_path.len() {
                    continue;
                }

                let prefix_matches = key_segments
                    .iter()
                    .zip(label_path.iter())
                    .all(|(k, lp)| clean_label(k).eq_ignore_ascii_case(&clean_label(lp)));

                if prefix_matches {
                    let rel_path = &label_path[key_segments.len()..];
                    if MenuNode::toggle_check_or_radio_by_path(nodes, rel_path, live_states) {
                        dirty = true;
                    }
                }
            }
            dirty
        };

        if modified {
            self.schedule_persist();
        }
    }

    /// Clears all cached AT-SPI2 submenus in RAM and removes the on-disk cache file if present.
    pub async fn clear_all(&self) {
        self.inner.write().await.clear();
        let path = self.cache_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if path.exists()
                && let Err(err) = fs::remove_file(&path) {
                    tracing::warn!(error = %err, path = %path.display(), "Failed removing AT-SPI2 disk cache");
                }
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::MenuItemType;

    fn sample_leaf(label: &str) -> MenuNode {
        MenuNode {
            id: 1,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Standard,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: Vec::new(),
        }
    }

    #[tokio::test]
    async fn atspi_cache_honors_disk_flag_and_persists_asynchronously() {
        let tmp_dir = std::env::temp_dir().join("genubar_test_atspi_cache");
        let _ = fs::remove_dir_all(&tmp_dir);
        let cache_file = tmp_dir.join("atspi_menus.json");

        let ram_only = AtspiSubmenuCache::load_with_options(cache_file.clone(), false);
        ram_only
            .store_submenu(
                "firefox",
                &[String::from("File")],
                vec![sample_leaf("New Tab")],
            )
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!cache_file.exists());
        assert!(ram_only.has_non_empty("firefox::File").await);

        let disk_cache = AtspiSubmenuCache::load_with_options(cache_file.clone(), true);
        disk_cache
            .store_submenu(
                "firefox",
                &[String::from("History")],
                vec![sample_leaf("Home")],
            )
            .await;
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(cache_file.is_file());

        let reloaded = AtspiSubmenuCache::load_with_options(cache_file.clone(), true);
        assert!(reloaded.has_non_empty("firefox::History").await);

        reloaded.clear_for_comm("firefox", None).await;
        assert!(!reloaded.has_non_empty("firefox::History").await);

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
