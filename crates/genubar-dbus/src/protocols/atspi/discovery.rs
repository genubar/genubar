use crate::protocols::atspi::parser::AtspiMenuParser;
use crate::protocols::atspi::proxy::AtspiAccessibleProxy;
use genubar_core::{AtspiSearchConfig, MenuError, MenuTarget};
use genubar_platform::proc_utils::{get_process_comm, pids_match};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use zbus::Connection;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

#[derive(Clone, Default)]
pub struct AtspiDiscovery {
    target_cache: Arc<RwLock<HashMap<u32, (AtspiSearchConfig, MenuTarget)>>>,
}

impl AtspiDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_search_rule() -> AtspiSearchConfig {
        AtspiSearchConfig {
            target_roles: vec![String::from("menu bar")],
            name_contains: None,
            traverse_document: false,
            hoist_single_button: false,
            max_depth: 6,
            max_nodes_budget: 85,
        }
    }

    pub async fn discover_target_by_pid(
        &self,
        a11y: &Connection,
        target_pid: u32,
    ) -> Option<MenuTarget> {
        self.discover_target_by_pid_with_config(a11y, target_pid, &Self::default_search_rule())
            .await
    }

    pub async fn invalidate_pid(&self, target_pid: u32) {
        if target_pid != 0 {
            self.target_cache.write().await.remove(&target_pid);
        }
    }

    /// Discovers an AT-SPI2 menu root for `target_pid` using the active search configuration.
    pub async fn discover_target_by_pid_with_config(
        &self,
        a11y: &Connection,
        target_pid: u32,
        rule: &AtspiSearchConfig,
    ) -> Option<MenuTarget> {
        if target_pid == 0 {
            return None;
        }
        if let Some((cached_rule, cached_target)) = self.target_cache.read().await.get(&target_pid)
            && cached_rule == rule
        {
            return Some(cached_target.clone());
        }
        let found = timeout(
            Duration::from_millis(450),
            self.discover_target_uncached(a11y, target_pid, rule),
        )
        .await
        .ok()
        .flatten();

        if let Some(ref valid_target) = found {
            self.target_cache
                .write()
                .await
                .insert(target_pid, (rule.clone(), valid_target.clone()));
        }
        found
    }

    async fn discover_target_uncached(
        &self,
        a11y: &Connection,
        target_pid: u32,
        rule: &AtspiSearchConfig,
    ) -> Option<MenuTarget> {
        let dbus_proxy = DBusProxy::new(a11y).await.ok()?;
        let root_proxy = AtspiAccessibleProxy::builder(a11y)
            .destination("org.a11y.atspi.Registry")
            .path("/org/a11y/atspi/accessible/root")
            .build()
            .await
            .ok()?;

        let app_nodes = root_proxy.get_children().await.ok()?;
        for (app_bus, app_path) in app_nodes {
            let Ok(bus_name) = BusName::try_from(app_bus.as_str()) else {
                continue;
            };
            let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus_name).await else {
                continue;
            };
            if pids_match(target_pid, pid)
                && let Some(menubar_path) =
                    Self::find_node_by_rule_bfs(a11y, &app_bus, app_path.as_str(), rule).await
            {
                return Some(MenuTarget::Atspi {
                    pid: target_pid,
                    app_bus_name: app_bus,
                    menubar_path,
                    hoist_single_button: rule.hoist_single_button,
                });
            }
        }
        None
    }

    async fn find_node_by_rule_bfs(
        a11y: &Connection,
        bus_name: &str,
        start_path: &str,
        rule: &AtspiSearchConfig,
    ) -> Option<String> {
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((start_path.to_string(), 0));
        let mut visited_count = 0usize;

        while let Some((curr_path, depth)) = queue.pop_front() {
            if visited_count > rule.max_nodes_budget || depth > rule.max_depth {
                continue;
            }
            visited_count += 1;

            let Ok(proxy) = AtspiAccessibleProxy::builder(a11y)
                .destination(bus_name)
                .path(curr_path.as_str())
                .build()
                .await
            else {
                continue;
            };

            let role = proxy.get_role_name().await.unwrap_or_default();
            if rule.target_roles.iter().any(|r| r == &role) {
                let name_matches = match &rule.name_contains {
                    Some(sub) => proxy
                        .name()
                        .await
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&sub.to_lowercase()),
                    None => true,
                };
                if name_matches {
                    return Some(curr_path);
                }
            }

            if !rule.traverse_document && AtspiMenuParser::is_heavy_container_role(&role) {
                continue;
            }

            if let Ok(children) = proxy.get_children().await {
                for (child_bus, child_path) in children {
                    if child_bus == bus_name {
                        queue.push_back((child_path.to_string(), depth + 1));
                    }
                }
            }
        }
        None
    }

    pub async fn dump_app_tree(
        a11y: &Connection,
        target_pid: u32,
        max_depth: usize,
        traverse_document: bool,
    ) -> Result<String, MenuError> {
        let dbus_proxy = DBusProxy::new(a11y)
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))?;
        let root_proxy = AtspiAccessibleProxy::builder(a11y)
            .destination("org.a11y.atspi.Registry")
            .path("/org/a11y/atspi/accessible/root")
            .build()
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))?;

        let app_nodes = root_proxy
            .get_children()
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))?;

        let mut out = String::new();
        let mut matched_any = false;

        for (app_bus, app_path) in app_nodes {
            let Ok(bus_name) = BusName::try_from(app_bus.as_str()) else {
                continue;
            };
            let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus_name).await else {
                continue;
            };
            if pids_match(target_pid, pid) {
                matched_any = true;
                out.push_str(&format!(
                    "=== AT-SPI2 App Node: bus={app_bus} (BusPID={pid}, TargetPID={target_pid}, comm='{}') root={app_path} ===\n",
                    get_process_comm(pid)
                ));
                let mut budget = 350usize;
                Self::dump_node_recursive(
                    a11y,
                    &app_bus,
                    app_path.as_str(),
                    0,
                    max_depth,
                    traverse_document,
                    &mut budget,
                    &mut out,
                )
                .await;
            }
        }

        if !matched_any {
            return Err(MenuError::NotFound(format!(
                "No AT-SPI2 application registered for PID {target_pid} (comm='{}').",
                get_process_comm(target_pid)
            )));
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    async fn dump_node_recursive(
        a11y: &Connection,
        bus_name: &str,
        object_path: &str,
        depth: usize,
        max_depth: usize,
        traverse_document: bool,
        budget: &mut usize,
        out: &mut String,
    ) {
        if depth > max_depth || *budget == 0 {
            return;
        }
        *budget -= 1;

        let Ok(proxy) = AtspiAccessibleProxy::builder(a11y)
            .destination(bus_name)
            .path(object_path)
            .build()
            .await
        else {
            return;
        };

        let role = proxy.get_role_name().await.unwrap_or_default();
        let name = proxy.name().await.unwrap_or_default();
        let children = proxy.get_children().await.unwrap_or_default();

        let indent = "  ".repeat(depth);
        out.push_str(&format!(
            "{indent}  role=\"{role}\" name=\"{name}\" children={} path={object_path}\n",
            children.len()
        ));

        if !traverse_document && AtspiMenuParser::is_heavy_container_role(&role) {
            return;
        }

        for (c_bus, c_path) in children {
            if c_bus == bus_name {
                Box::pin(Self::dump_node_recursive(
                    a11y,
                    bus_name,
                    c_path.as_str(),
                    depth + 1,
                    max_depth,
                    traverse_document,
                    budget,
                    out,
                ))
                .await;
            }
        }
    }
}
