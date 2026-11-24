use super::parser::CanonicalMenuParser;
use super::proxy::CanonicalDbusMenuProxy;
use async_trait::async_trait;
use genubar_core::errors::MenuError;
use genubar_core::{MenuNode, MenuProvider, MenuTarget, clean_label};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use zbus::{Connection, Variant};

const MAX_CACHED_TARGETS: usize = 64;

/// Tuning parameters propagated from `CanonicalDbusTierConfig`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalFetchOptions {
    pub preload_passes: u8,
    pub swing_open_delay_ms: u64,
}

impl Default for CanonicalFetchOptions {
    fn default() -> Self {
        Self {
            preload_passes: 2,
            swing_open_delay_ms: 35,
        }
    }
}

fn now_unix_timestamp_u32() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

type TargetPathMap = HashMap<i32, Vec<String>>;

#[derive(Clone)]
pub struct CanonicalMenuClient {
    connection: Connection,
    path_caches: Arc<RwLock<HashMap<(String, String), TargetPathMap>>>,
    next_uid: Arc<AtomicI32>,
}

impl CanonicalMenuClient {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            path_caches: Arc::new(RwLock::new(HashMap::new())),
            next_uid: Arc::new(AtomicI32::new(1)),
        }
    }

    fn target_key(target: &MenuTarget) -> Result<(String, String), MenuError> {
        match target {
            MenuTarget::Canonical {
                bus_name,
                object_path,
            } => Ok((bus_name.clone(), object_path.clone())),
            _ => Err(MenuError::Transport(
                "CanonicalMenuClient requires a Canonical target".into(),
            )),
        }
    }

    async fn store_target_paths(&self, key: (String, String), paths: TargetPathMap) {
        let mut caches = self.path_caches.write().await;
        if caches.len() >= MAX_CACHED_TARGETS
            && !caches.contains_key(&key)
            && let Some(evict_key) = caches.keys().next().cloned()
        {
            caches.remove(&evict_key);
        }
        caches.insert(key, paths);
    }

    fn collect_existing_menu_paths(
        node: &MenuNode,
        is_root: bool,
        current_path: &mut Vec<String>,
        map: &mut TargetPathMap,
    ) {
        if !is_root {
            let clean = clean_label(&node.label);
            if clean.is_empty() {
                return;
            }
            current_path.push(clean);
            if node.id > 0 {
                let resolved = node
                    .origin_path
                    .as_ref()
                    .filter(|p| !p.is_empty())
                    .cloned()
                    .unwrap_or_else(|| current_path.clone());
                map.entry(node.id).or_insert(resolved);
            }
        }
        for child in &node.children {
            Self::collect_existing_menu_paths(child, false, current_path, map);
        }
        if !is_root {
            current_path.pop();
        }
    }

    /// Populates `path_caches` from a cached `MenuNode` tree (e.g., Stage 1 L1 skeleton) so
    /// subsequent `FetchSubmenu(id)` calls can still walk live Canonical D-Bus nodes by label path.
    pub async fn seed_target_paths_from_menu(&self, target: &MenuTarget, root: &MenuNode) {
        let Ok(key) = Self::target_key(target) else {
            return;
        };
        let mut seeded = HashMap::new();
        let mut curr = Vec::new();
        Self::collect_existing_menu_paths(root, true, &mut curr, &mut seeded);
        if seeded.is_empty() {
            return;
        }
        let mut caches = self.path_caches.write().await;
        if caches.len() >= MAX_CACHED_TARGETS
            && !caches.contains_key(&key)
            && let Some(evict_key) = caches.keys().next().cloned()
        {
            caches.remove(&evict_key);
        }
        let entry = caches.entry(key).or_default();
        for (id, path) in seeded {
            entry.entry(id).or_insert(path);
        }
    }

    fn allocate_uid_counter(&self) {
        let current = self.next_uid.load(Ordering::Relaxed);
        if current >= 1_000_000_000 {
            self.next_uid.store(1, Ordering::Relaxed);
        }
    }

    async fn build_proxy<'a>(
        &self,
        target: &'a MenuTarget,
    ) -> Result<CanonicalDbusMenuProxy<'a>, MenuError> {
        let MenuTarget::Canonical {
            bus_name,
            object_path,
        } = target
        else {
            return Err(MenuError::Transport(
                "CanonicalMenuClient requires a Canonical target".into(),
            ));
        };

        CanonicalDbusMenuProxy::builder(&self.connection)
            .destination(bus_name.as_str())
            .path(object_path.as_str())
            .build()
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))
    }

    async fn resolve_live_id_by_path(
        &self,
        proxy: &CanonicalDbusMenuProxy<'_>,
        path: &[String],
        open_delay_ms: u64,
    ) -> Option<i32> {
        let mut current_parent_id = 0i32;
        let now = now_unix_timestamp_u32();
        let wait_ms = open_delay_ms.clamp(5, 250);

        for (depth_idx, target_label) in path.iter().enumerate() {
            let mut layout_res = proxy.get_layout(current_parent_id, 1, vec![]).await.ok()?;
            let mut parsed_parent = CanonicalMenuParser::parse_node(
                layout_res.1.0,
                depth_idx,
                &layout_res.1.1,
                &layout_res.1.2,
            )
            .ok()?;

            if !parsed_parent
                .children
                .iter()
                .any(|c| clean_label(&c.label) == *target_label)
            {
                let _ = proxy.about_to_show(current_parent_id).await;
                if current_parent_id != 0 {
                    let _ = proxy
                        .event(current_parent_id, "opened", &Variant::I32(0), now)
                        .await;
                }
                sleep(Duration::from_millis(wait_ms)).await;
                layout_res = proxy.get_layout(current_parent_id, 1, vec![]).await.ok()?;
                parsed_parent = CanonicalMenuParser::parse_node(
                    layout_res.1.0,
                    depth_idx,
                    &layout_res.1.1,
                    &layout_res.1.2,
                )
                .ok()?;
            }

            let matched_child = parsed_parent
                .children
                .into_iter()
                .find(|c| clean_label(&c.label) == *target_label)?;

            current_parent_id = matched_child.id;
        }
        Some(current_parent_id)
    }

    /// Fetches the root Canonical menu honoring configured preload passes and Swing poll delay.
    pub async fn fetch_menu_with_options(
        &self,
        target: &MenuTarget,
        options: CanonicalFetchOptions,
    ) -> Result<MenuNode, MenuError> {
        let cache_key = Self::target_key(target)?;
        let proxy = self.build_proxy(target).await?;
        let _ = proxy.about_to_show(0).await;

        let max_attempts = (options.preload_passes as usize)
            .saturating_add(1)
            .clamp(1, 6);
        let retry_delay_ms = options.swing_open_delay_ms.clamp(10, 500);

        let mut tree = None;
        let mut transport_err: Option<String> = None;
        for attempt in 0..max_attempts {
            // Try full recursive fetch quickly; fall back to depth=1 if a heavy Swing/Electron tree stalls.
            let layout_res =
                match timeout(Duration::from_millis(450), proxy.get_layout(0, -1, vec![])).await {
                    Ok(Ok(res)) => Ok(res),
                    Ok(Err(e)) => {
                        transport_err = Some(e.to_string());
                        Err(())
                    }
                    Err(_) => {
                        match timeout(Duration::from_millis(350), proxy.get_layout(0, 1, vec![]))
                            .await
                        {
                            Ok(Ok(res)) => Ok(res),
                            Ok(Err(e)) => {
                                transport_err = Some(e.to_string());
                                Err(())
                            }
                            Err(_) => Err(()),
                        }
                    }
                };

            if let Ok((_revision, (root_id, root_props, root_children))) = layout_res {
                transport_err = None;
                if let Ok(parsed) =
                    CanonicalMenuParser::parse_node(root_id, 0, &root_props, &root_children)
                {
                    let has_cols = !parsed.children.is_empty();
                    tree = Some(parsed);
                    if has_cols {
                        break;
                    }
                }
            }
            if attempt + 1 < max_attempts {
                sleep(Duration::from_millis(retry_delay_ms)).await;
                let _ = proxy.about_to_show(0).await;
            }
        }

        let mut tree = match tree {
            Some(t) => t,
            None => {
                if let Some(err) = transport_err {
                    return Err(MenuError::Transport(err));
                }
                return Err(MenuError::NotFound("Failed to parse Canonical root".into()));
            }
        };

        if tree.children.is_empty() {
            return Err(MenuError::NotFound(
                "Canonical menu root has 0 top-level columns".into(),
            ));
        }

        self.allocate_uid_counter();
        let mut local_paths = HashMap::new();
        let mut curr_path = Vec::new();
        CanonicalMenuParser::assign_uids_and_index_paths(
            &mut tree,
            true,
            &self.next_uid,
            &mut curr_path,
            &mut local_paths,
        );
        self.store_target_paths(cache_key, local_paths).await;

        Ok(tree)
    }

    /// Fetches a lazy Canonical submenu on demand using the configured Swing open polling interval.
    pub async fn fetch_submenu_with_options(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
        options: CanonicalFetchOptions,
    ) -> Result<MenuNode, MenuError> {
        let cache_key = Self::target_key(target)?;
        let cached_path = self
            .path_caches
            .read()
            .await
            .get(&cache_key)
            .and_then(|m| m.get(&submenu_id))
            .cloned()
            .ok_or_else(|| MenuError::NotFound(format!("Unknown submenu ID {submenu_id}")))?;

        self.fetch_submenu_for_cached_path(target, submenu_id, cached_path, options)
            .await
    }

    /// Fallback submenu resolver when an ID came from an older L0/L1 snapshot but its label path is known.
    pub async fn fetch_submenu_by_path_with_options(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
        cached_path: Vec<String>,
        options: CanonicalFetchOptions,
    ) -> Result<MenuNode, MenuError> {
        if cached_path.is_empty() {
            return Err(MenuError::NotFound(format!(
                "Empty fallback path for submenu ID {submenu_id}"
            )));
        }
        self.fetch_submenu_for_cached_path(target, submenu_id, cached_path, options)
            .await
    }

    async fn fetch_submenu_for_cached_path(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
        cached_path: Vec<String>,
        options: CanonicalFetchOptions,
    ) -> Result<MenuNode, MenuError> {
        let cache_key = Self::target_key(target)?;
        let proxy = self.build_proxy(target).await?;
        let now = now_unix_timestamp_u32();

        let open_delay_ms = options.swing_open_delay_ms.clamp(5, 250);
        let live_id = self
            .resolve_live_id_by_path(&proxy, &cached_path, open_delay_ms)
            .await
            .ok_or_else(|| {
                MenuError::NotFound(format!("Failed to resolve live path {cached_path:?}"))
            })?;

        let depth = cached_path.len();
        let _ = proxy.about_to_show(live_id).await;
        let _ = proxy.event(live_id, "opened", &Variant::I32(0), now).await;

        let poll_attempts = (options.preload_passes as usize)
            .saturating_add(6)
            .clamp(4, 12);
        let poll_interval = Duration::from_millis(open_delay_ms.min(60));
        let mut subtree = None;
        for _attempt in 0..poll_attempts {
            if let Ok((_rev, (r_id, r_props, r_children))) =
                proxy.get_layout(live_id, -1, vec![]).await
                && let Ok(parsed) =
                    CanonicalMenuParser::parse_node(r_id, depth, &r_props, &r_children)
            {
                if CanonicalMenuParser::has_meaningful_children(&parsed) {
                    subtree = Some(parsed);
                    break;
                }
                subtree = Some(parsed);
            }
            sleep(poll_interval).await;
        }

        let mut node = subtree
            .ok_or_else(|| MenuError::NotFound(format!("Submenu {submenu_id} not found")))?;

        let mut caches = self.path_caches.write().await;
        let target_map = caches.entry(cache_key).or_default();
        let mut prefix = cached_path;
        for child in &mut node.children {
            CanonicalMenuParser::assign_uids_and_index_paths(
                child,
                false,
                &self.next_uid,
                &mut prefix,
                target_map,
            );
        }
        node.id = submenu_id;

        Ok(node)
    }
}

#[async_trait]
impl MenuProvider for CanonicalMenuClient {
    async fn fetch_menu(&self, target: &MenuTarget) -> Result<MenuNode, MenuError> {
        self.fetch_menu_with_options(target, CanonicalFetchOptions::default())
            .await
    }

    async fn fetch_submenu(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
    ) -> Result<MenuNode, MenuError> {
        self.fetch_submenu_with_options(target, submenu_id, CanonicalFetchOptions::default())
            .await
    }

    async fn activate_item(&self, target: &MenuTarget, item_id: i32) -> Result<(), MenuError> {
        let cache_key = Self::target_key(target)?;
        let proxy = self.build_proxy(target).await?;
        let now = now_unix_timestamp_u32();

        let cached_path = self
            .path_caches
            .read()
            .await
            .get(&cache_key)
            .and_then(|m| m.get(&item_id))
            .cloned();

        let target_id = if let Some(ref path) = cached_path {
            self.resolve_live_id_by_path(
                &proxy,
                path,
                CanonicalFetchOptions::default().swing_open_delay_ms,
            )
            .await
            .unwrap_or(item_id)
        } else {
            item_id
        };

        proxy
            .event(target_id, "clicked", &Variant::I32(0), now)
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_cache_keys_isolate_concurrent_windows() {
        let win_a = MenuTarget::Canonical {
            bus_name: String::from(":1.10"),
            object_path: String::from("/MenuBar/1"),
        };
        let win_b = MenuTarget::Canonical {
            bus_name: String::from(":1.20"),
            object_path: String::from("/MenuBar/2"),
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let conn = match Connection::session().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let client = CanonicalMenuClient::new(conn);
            let key_a = CanonicalMenuClient::target_key(&win_a).unwrap();
            let key_b = CanonicalMenuClient::target_key(&win_b).unwrap();

            client
                .store_target_paths(
                    key_a.clone(),
                    HashMap::from([(1, vec![String::from("File")])]),
                )
                .await;
            client
                .store_target_paths(
                    key_b.clone(),
                    HashMap::from([(2, vec![String::from("Edit")])]),
                )
                .await;

            let caches = client.path_caches.read().await;
            assert_eq!(caches.get(&key_a).unwrap().get(&1).unwrap(), &vec!["File"]);
            assert_eq!(caches.get(&key_b).unwrap().get(&2).unwrap(), &vec!["Edit"]);
        });
    }

    #[test]
    fn collect_existing_menu_paths_preserves_origin_paths_and_labels() {
        let root = MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: genubar_core::MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![MenuNode {
                id: 10,
                label: String::from("_File Renamed"),
                enabled: true,
                visible: true,
                item_type: genubar_core::MenuItemType::Submenu,
                shortcut: None,
                action: None,
                icon: None,
                origin_path: Some(vec![String::from("File")]),
                children: vec![MenuNode {
                    id: 11,
                    label: String::from("_Save"),
                    enabled: true,
                    visible: true,
                    item_type: genubar_core::MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: vec![],
                }],
            }],
        };

        let mut map = HashMap::new();
        let mut curr = Vec::new();
        CanonicalMenuClient::collect_existing_menu_paths(&root, true, &mut curr, &mut map);
        assert_eq!(map.get(&10).unwrap(), &vec![String::from("File")]);
        assert_eq!(
            map.get(&11).unwrap(),
            &vec![String::from("File Renamed"), String::from("Save")]
        );
    }
}
