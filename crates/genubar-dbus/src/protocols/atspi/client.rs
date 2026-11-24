use super::a11y_bus::A11yBusClient;
use super::cache::AtspiSubmenuCache;
use super::discovery::AtspiDiscovery;
use super::menu_opener::{open_path_chain, poll_target_subtree, trigger_node_action};
use super::state_probe::read_live_sibling_states;
use super::tree_walker::{AtspiTraversalContext, build_subtree};
use crate::models::atspi::AtspiActionRef;
use async_trait::async_trait;
use genubar_core::{
    AtspiSearchConfig, MenuError, MenuItemType, MenuNode, MenuProvider, MenuTarget,
};
use genubar_platform::proc_utils::get_process_comm;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use zbus::Connection;

const MAX_TARGET_ID_CACHES: usize = 32;

#[derive(Default, Clone)]
struct AtspiTargetIdCache {
    actions: HashMap<i32, AtspiActionRef>,
    paths: HashMap<i32, Vec<String>>,
}

#[derive(Clone)]
pub struct AtspiMenuClient {
    a11y_bus: A11yBusClient,
    target_caches: Arc<RwLock<HashMap<(String, String), AtspiTargetIdCache>>>,
    discovery: AtspiDiscovery,
    submenu_cache: AtspiSubmenuCache,
    next_uid: Arc<AtomicI32>,
    op_lock: Arc<Mutex<()>>,
}

impl AtspiMenuClient {
    pub fn new(session_conn: Connection) -> Self {
        Self {
            a11y_bus: A11yBusClient::new(session_conn),
            target_caches: Arc::new(RwLock::new(HashMap::new())),
            discovery: AtspiDiscovery::new(),
            submenu_cache: AtspiSubmenuCache::load_or_new(),
            next_uid: Arc::new(AtomicI32::new(1)),
            op_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn set_disk_cache_enabled(&self, enabled: bool) {
        self.submenu_cache.set_disk_cache_enabled(enabled);
    }

    pub async fn clear_cache_for_pid(&self, pid: u32, only_submenus: Option<&[String]>) {
        self.submenu_cache.clear_for_pid(pid, only_submenus).await;
    }

    pub async fn clear_all_caches(&self) {
        self.target_caches.write().await.clear();
        self.submenu_cache.clear_all().await;
    }

    pub async fn invalidate_target_cache(&self, target_pid: u32) {
        self.discovery.invalidate_pid(target_pid).await;
    }

    pub async fn fetch_submenu_by_path(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
        label_path: Vec<String>,
    ) -> Result<MenuNode, MenuError> {
        if label_path.is_empty() {
            return Err(MenuError::NotFound(format!(
                "Empty fallback path for AT-SPI2 submenu {submenu_id}"
            )));
        }
        self.fetch_submenu_for_label_path(target, submenu_id, label_path)
            .await
    }

    pub async fn discover_target_by_pid(&self, target_pid: u32) -> Option<MenuTarget> {
        let a11y = self.a11y_bus.connection().await.ok()?;
        self.discovery
            .discover_target_by_pid(a11y, target_pid)
            .await
    }

    pub async fn discover_target_by_pid_with_config(
        &self,
        target_pid: u32,
        rule: &AtspiSearchConfig,
    ) -> Option<MenuTarget> {
        let a11y = self.a11y_bus.connection().await.ok()?;
        self.discovery
            .discover_target_by_pid_with_config(a11y, target_pid, rule)
            .await
    }

    pub async fn dump_app_tree(
        &self,
        target_pid: u32,
        max_depth: usize,
        traverse_document: bool,
    ) -> Result<String, MenuError> {
        let a11y = self.a11y_bus.connection().await?;
        AtspiDiscovery::dump_app_tree(a11y, target_pid, max_depth, traverse_document).await
    }

    async fn replace_target_cache(
        &self,
        key: (String, String),
        actions: HashMap<i32, AtspiActionRef>,
        paths: HashMap<i32, Vec<String>>,
    ) {
        let mut caches = self.target_caches.write().await;
        if caches.len() >= MAX_TARGET_ID_CACHES
            && !caches.contains_key(&key)
            && let Some(evict) = caches.keys().next().cloned()
        {
            caches.remove(&evict);
        }
        caches.insert(key, AtspiTargetIdCache { actions, paths });
    }

    async fn extend_target_cache(
        &self,
        key: (String, String),
        actions: HashMap<i32, AtspiActionRef>,
        paths: HashMap<i32, Vec<String>>,
    ) {
        let mut caches = self.target_caches.write().await;
        let entry = caches.entry(key).or_default();
        entry.actions.extend(actions);
        entry.paths.extend(paths);
    }

    fn normalize_uid_counter(&self) {
        if self.next_uid.load(Ordering::Relaxed) >= 1_000_000_000 {
            self.next_uid.store(1, Ordering::Relaxed);
        }
    }
}

#[async_trait]
impl MenuProvider for AtspiMenuClient {
    async fn fetch_menu(&self, target: &MenuTarget) -> Result<MenuNode, MenuError> {
        let MenuTarget::Atspi {
            pid,
            app_bus_name,
            menubar_path,
            ..
        } = target
        else {
            return Err(MenuError::Transport(
                "AtspiMenuClient requires an Atspi target".into(),
            ));
        };

        let _guard = self.op_lock.lock().await;
        let app_comm = get_process_comm(*pid);
        let a11y = self.a11y_bus.connection().await?;
        self.normalize_uid_counter();

        let mut ctx = AtspiTraversalContext {
            submenu_cache: &self.submenu_cache,
            next_uid: &self.next_uid,
            app_comm: &app_comm,
            menubar_root: menubar_path,
            a11y,
            bus_name: app_bus_name,
            expand_nested_menus: false,
            current_path: Vec::new(),
            action_cache: HashMap::new(),
            paths_map: HashMap::new(),
        };
        let mut root_budget = 1000usize;

        let mut root = timeout(
            Duration::from_millis(900),
            build_subtree(&mut ctx, menubar_path, 0, &mut root_budget),
        )
        .await
        .ok()
        .flatten()
        .ok_or_else(|| {
            MenuError::NotFound(format!("Failed to read AT-SPI2 menu bar at {menubar_path}"))
        })?;

        root.id = 0;
        self.replace_target_cache(
            (app_bus_name.clone(), menubar_path.clone()),
            ctx.action_cache,
            ctx.paths_map,
        )
        .await;

        Ok(root)
    }

    async fn fetch_submenu(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
    ) -> Result<MenuNode, MenuError> {
        let MenuTarget::Atspi {
            app_bus_name,
            menubar_path,
            ..
        } = target
        else {
            return Err(MenuError::Transport(
                "AtspiMenuClient requires an Atspi target".into(),
            ));
        };

        let target_key = (app_bus_name.clone(), menubar_path.clone());
        let label_path = self
            .target_caches
            .read()
            .await
            .get(&target_key)
            .and_then(|c| c.paths.get(&submenu_id))
            .cloned()
            .ok_or_else(|| MenuError::NotFound(format!("Unknown AT-SPI2 submenu {submenu_id}")))?;

        self.fetch_submenu_for_label_path(target, submenu_id, label_path)
            .await
    }

    async fn activate_item(&self, target: &MenuTarget, item_id: i32) -> Result<(), MenuError> {
        let MenuTarget::Atspi {
            pid,
            app_bus_name,
            menubar_path,
            ..
        } = target
        else {
            return Err(MenuError::Transport(
                "AtspiMenuClient requires an Atspi target".into(),
            ));
        };

        let target_key = (app_bus_name.clone(), menubar_path.clone());
        let has_item = self
            .target_caches
            .read()
            .await
            .get(&target_key)
            .is_some_and(|c| c.actions.contains_key(&item_id));
        if !has_item {
            let _ = self.fetch_menu(target).await?;
        }

        let (act_ref, label_path_opt) = {
            let caches = self.target_caches.read().await;
            let entry = caches.get(&target_key);
            let act_ref = entry
                .and_then(|c| c.actions.get(&item_id))
                .cloned()
                .ok_or_else(|| {
                    MenuError::NotFound(format!("AT-SPI2 item ID {item_id} not found"))
                })?;
            let path = entry.and_then(|c| c.paths.get(&item_id)).cloned();
            (act_ref, path)
        };

        let _guard = self.op_lock.lock().await;
        let a11y = self.a11y_bus.connection().await?;
        let mut action_succeeded = false;

        match act_ref {
            AtspiActionRef::Direct {
                ref bus_name,
                ref object_path,
                ..
            } => {
                action_succeeded = trigger_node_action(
                    a11y,
                    bus_name,
                    object_path,
                    &format!("ActivateItem Direct (id={item_id})"),
                )
                .await;

                if !action_succeeded
                    && let Some(ref lpath) = label_path_opt
                    && let Some(fresh_leaf) =
                        open_path_chain(a11y, bus_name, menubar_path, lpath, true).await
                {
                    action_succeeded = trigger_node_action(
                        a11y,
                        bus_name,
                        &fresh_leaf,
                        &format!("ActivateItem Direct-Fallback (id={item_id})"),
                    )
                    .await;
                }
            }
            AtspiActionRef::CachedReplay {
                ref bus_name,
                menubar_path: ref mb_path,
                ref label_path,
            } => {
                if let Some(leaf_path) =
                    open_path_chain(a11y, bus_name, mb_path, label_path, false).await
                {
                    action_succeeded = trigger_node_action(
                        a11y,
                        bus_name,
                        &leaf_path,
                        &format!("ActivateItem Replay Phase 1 (id={item_id})"),
                    )
                    .await;
                }

                if !action_succeeded
                    && let Some(fresh_leaf) =
                        open_path_chain(a11y, bus_name, mb_path, label_path, true).await
                {
                    action_succeeded = trigger_node_action(
                        a11y,
                        bus_name,
                        &fresh_leaf,
                        &format!("ActivateItem Replay Phase 2 (id={item_id})"),
                    )
                    .await;
                }
            }
        }

        if !action_succeeded {
            return Err(MenuError::Transport(format!(
                "AT-SPI2 DoAction(0) failed for item ID {item_id}"
            )));
        }

        if let Some(lpath) = label_path_opt
            && lpath.len() >= 2
        {
            tokio::time::sleep(Duration::from_millis(35)).await;
            let parent_path = &lpath[..lpath.len() - 1];
            let live_states =
                read_live_sibling_states(a11y, app_bus_name, menubar_path, parent_path).await;

            self.submenu_cache
                .update_toggle_state_by_path(*pid, &lpath, live_states.as_deref())
                .await;
        }

        Ok(())
    }
}

impl AtspiMenuClient {
    async fn fetch_submenu_for_label_path(
        &self,
        target: &MenuTarget,
        submenu_id: i32,
        label_path: Vec<String>,
    ) -> Result<MenuNode, MenuError> {
        let MenuTarget::Atspi {
            pid,
            app_bus_name,
            menubar_path,
            ..
        } = target
        else {
            return Err(MenuError::Transport(
                "AtspiMenuClient requires an Atspi target".into(),
            ));
        };
        let target_key = (app_bus_name.clone(), menubar_path.clone());
        let app_comm = get_process_comm(*pid);

        let mut local_cache = HashMap::new();
        let mut local_paths = HashMap::new();
        let mut prefix = label_path.clone();

        if let Some(cached_children) = self
            .submenu_cache
            .restore_and_rebind(
                &app_comm,
                &format!("{app_comm}::{}", label_path.join("::")),
                &self.next_uid,
                app_bus_name,
                menubar_path,
                &mut prefix,
                &mut local_cache,
                &mut local_paths,
            )
            .await
        {
            tracing::debug!(
                submenu_id,
                items = cached_children.len(),
                "AT-SPI2 FetchSubmenu cache hit"
            );
            self.extend_target_cache(target_key, local_cache, local_paths)
                .await;
            return Ok(MenuNode {
                id: submenu_id,
                label: label_path.last().cloned().unwrap_or_default(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                origin_path: None,
                children: cached_children,
                icon: None,
            });
        }

        let _guard = self.op_lock.lock().await;
        let a11y = self.a11y_bus.connection().await?;
        let is_nested = label_path.len() >= 2;
        let expand_nested = label_path.len() == 1;

        let walk_fut = async {
            let target_obj_path =
                open_path_chain(a11y, app_bus_name, menubar_path, &label_path, false)
                    .await
                    .ok_or_else(|| {
                        MenuError::NotFound(format!(
                            "Could not resolve AT-SPI2 path {label_path:?}"
                        ))
                    })?;

            trigger_node_action(
                a11y,
                app_bus_name,
                &target_obj_path,
                &format!("Phase 1 Target (id={submenu_id})"),
            )
            .await;

            let phase1_polls = if is_nested { 2 } else { 6 };
            let (mut parsed_node, mut final_cache, mut final_paths) = poll_target_subtree(
                &self.submenu_cache,
                &self.next_uid,
                &app_comm,
                menubar_path,
                a11y,
                app_bus_name,
                &target_obj_path,
                &label_path,
                expand_nested,
                phase1_polls,
                submenu_id,
            )
            .await;

            let still_empty = parsed_node
                .as_ref()
                .map(|n| n.children.is_empty())
                .unwrap_or(true);

            if is_nested
                && still_empty
                && let Some(fresh_target_path) =
                    open_path_chain(a11y, app_bus_name, menubar_path, &label_path, true).await
            {
                trigger_node_action(
                    a11y,
                    app_bus_name,
                    &fresh_target_path,
                    &format!("Phase 2 Target (id={submenu_id})"),
                )
                .await;

                let (node2, cache2, paths2) = poll_target_subtree(
                    &self.submenu_cache,
                    &self.next_uid,
                    &app_comm,
                    menubar_path,
                    a11y,
                    app_bus_name,
                    &fresh_target_path,
                    &label_path,
                    false,
                    6,
                    submenu_id,
                )
                .await;

                if node2.is_some() {
                    parsed_node = node2;
                    final_cache = cache2;
                    final_paths = paths2;
                }
            }

            let mut node = parsed_node
                .ok_or_else(|| MenuError::NotFound("Failed to parse AT-SPI2 submenu".into()))?;
            node.id = submenu_id;
            Ok::<_, MenuError>((node, final_cache, final_paths))
        };

        let (node, final_cache, final_paths) = timeout(Duration::from_secs(7), walk_fut)
            .await
            .map_err(|_| {
            MenuError::Transport(format!("Timed out fetching AT-SPI2 submenu {label_path:?}"))
        })??;

        self.extend_target_cache(target_key, final_cache, final_paths)
            .await;

        Ok(node)
    }
}
