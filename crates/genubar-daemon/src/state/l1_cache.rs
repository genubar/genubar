use genubar_core::{ActiveWindowInfo, MenuNode};
use genubar_platform::cache_dir;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, RwLock};

type SkeletonMap = HashMap<String, MenuNode>;

/// Persistent L1 top-level menu header cache for sub-millisecond panel rendering on focus switch.
/// Disk serialization runs in a debounced background worker outside the `RwLock`.
#[derive(Clone)]
pub struct L1SkeletonCache {
    inner: Arc<RwLock<SkeletonMap>>,
    cache_path: PathBuf,
    flush_notify: Arc<Notify>,
    worker_started: Arc<AtomicBool>,
}

impl L1SkeletonCache {
    pub fn load_or_new() -> Self {
        Self::load_from_path(cache_dir().join("l1_roots.json"))
    }

    pub fn load_from_path(cache_path: PathBuf) -> Self {
        let map = Self::load_from_disk(&cache_path);
        Self {
            inner: Arc::new(RwLock::new(map)),
            cache_path,
            flush_notify: Arc::new(Notify::new()),
            worker_started: Arc::new(AtomicBool::new(false)),
        }
    }

    fn load_from_disk(path: &PathBuf) -> SkeletonMap {
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(map) = serde_json::from_str::<SkeletonMap>(&raw) {
                tracing::debug!(count = map.len(), path = %path.display(), "Loaded L1 menu skeletons from disk");
                return map;
            }
        HashMap::new()
    }

    fn write_snapshot_to_disk(path: &PathBuf, map: &SkeletonMap) {
        if map.is_empty() {
            if path.exists()
                && let Err(err) = fs::remove_file(path) {
                    tracing::warn!(error = %err, path = %path.display(), "Failed removing empty L1 cache file");
                }
            return;
        }
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent) {
                tracing::warn!(error = %err, path = %parent.display(), "Failed to create L1 cache directory");
                return;
            }
        match serde_json::to_string_pretty(map) {
            Ok(json) => {
                if let Err(err) = fs::write(path, json) {
                    tracing::warn!(error = %err, path = %path.display(), "Failed to persist L1 skeleton cache");
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed to serialize L1 skeleton cache");
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
        let path = self.cache_path.clone();
        let notify = self.flush_notify.clone();

        handle.spawn(async move {
            loop {
                notify.notified().await;
                tokio::time::sleep(Duration::from_millis(250)).await;
                let snapshot = inner.read().await.clone();
                let path_c = path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    Self::write_snapshot_to_disk(&path_c, &snapshot);
                })
                .await;
            }
        });
    }

    pub fn compute_l1_key(info: &ActiveWindowInfo) -> Option<String> {
        let app = info.app_id.trim().to_lowercase();
        if app.is_empty() || info.pid == 0 {
            return None;
        }
        let short_app_name = app
            .trim_end_matches(".desktop")
            .rsplit('.')
            .next()
            .unwrap_or(&app);
        let title_low = info.title.trim().to_lowercase();
        let is_welcome_or_splash = !title_low.is_empty()
            && (title_low == short_app_name
                || title_low.starts_with("welcome ")
                || title_low.contains("quick start")
                || title_low.contains("select project"));

        let role = if is_welcome_or_splash {
            "welcome"
        } else {
            "main"
        };
        Some(format!("{app}::{role}"))
    }

    pub async fn store_menu(&self, info: &ActiveWindowInfo, root: &MenuNode) {
        let Some(key) = Self::compute_l1_key(info) else {
            return;
        };
        if root.children.is_empty() {
            return;
        }
        let mut l1_skeleton = root.clone();
        for col in &mut l1_skeleton.children {
            if col.children.len() > 25 {
                col.children.clear();
            }
        }
        {
            let mut guard = self.inner.write().await;
            guard.insert(key, l1_skeleton);
        }
        self.ensure_background_worker();
        self.flush_notify.notify_one();
    }

    pub async fn get_menu(&self, info: &ActiveWindowInfo) -> Option<MenuNode> {
        let key = Self::compute_l1_key(info)?;
        self.inner.read().await.get(&key).cloned()
    }

    /// Clears all in-memory L1 skeletons and deletes `l1_roots.json` on disk.
    pub async fn clear_all(&self) {
        self.inner.write().await.clear();
        let path = self.cache_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if path.exists()
                && let Err(err) = fs::remove_file(&path) {
                    tracing::warn!(error = %err, path = %path.display(), "Failed removing L1 skeleton cache");
                }
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::MenuItemType;

    fn sample_menu() -> MenuNode {
        MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![MenuNode {
                id: 1,
                label: String::from("File"),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                icon: None,
                origin_path: None,
                children: Vec::new(),
            }],
        }
    }

    #[test]
    fn compute_l1_key_distinguishes_splash_and_document_without_hardcoded_apps() {
        let no_pid = ActiveWindowInfo {
            app_id: String::from("org.example.App"),
            pid: 0,
            ..Default::default()
        };
        assert!(L1SkeletonCache::compute_l1_key(&no_pid).is_none());

        let splash = ActiveWindowInfo {
            pid: 123,
            app_id: String::from("org.example.Painter.desktop"),
            title: String::from("Painter"),
            ..Default::default()
        };
        assert_eq!(
            L1SkeletonCache::compute_l1_key(&splash).as_deref(),
            Some("org.example.painter.desktop::welcome")
        );

        let main_doc = ActiveWindowInfo {
            pid: 123,
            app_id: String::from("org.example.Painter.desktop"),
            title: String::from("Drawing.svg - Painter"),
            ..Default::default()
        };
        assert_eq!(
            L1SkeletonCache::compute_l1_key(&main_doc).as_deref(),
            Some("org.example.painter.desktop::main")
        );
    }

    #[tokio::test]
    async fn l1_cache_persists_skeleton_asynchronously() {
        let tmp_dir = std::env::temp_dir().join("genubar_test_l1_cache");
        let _ = fs::remove_dir_all(&tmp_dir);
        let cache_file = tmp_dir.join("l1_roots.json");

        let cache = L1SkeletonCache::load_from_path(cache_file.clone());
        let info = ActiveWindowInfo {
            pid: 42,
            app_id: String::from("org.gnome.Nautilus"),
            title: String::from("Home"),
            ..Default::default()
        };
        cache.store_menu(&info, &sample_menu()).await;
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(cache_file.is_file());

        let reloaded = L1SkeletonCache::load_from_path(cache_file);
        assert!(reloaded.get_menu(&info).await.is_some());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
