use super::CachedGtkAction;
use super::actions::{activate_custom_gtk_action, activate_org_gtk_action, fetch_action_states};
use super::hook_bridge::{
    dump_gtk4_introspection, fetch_hook_action_states, query_gtk4_zoom_percent,
    try_activate_bridge_action,
};
use super::parser::GtkMenuParser;
use super::proxy::GtkMenusProxy;
use async_trait::async_trait;
use genubar_core::errors::MenuError;
use genubar_core::{ActiveWindowInfo, MenuItemType, MenuNode, MenuProvider, MenuTarget};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::{Connection, OwnedVariant};

type GtkActionCacheKey = (String, String, i32);

#[derive(Clone)]
pub struct GtkMenuClient {
    connection: Connection,
    action_cache: Arc<RwLock<HashMap<GtkActionCacheKey, CachedGtkAction>>>,
}

impl GtkMenuClient {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            action_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn target_cache_scope(menubar_path: Option<&str>, win_path: Option<&str>) -> String {
        format!(
            "{}|{}",
            menubar_path.unwrap_or_default(),
            win_path.unwrap_or_default()
        )
    }

    async fn fetch_all_menu_groups(
        &self,
        bus_name: &str,
        object_path: &str,
    ) -> Result<HashMap<(u32, u32), Vec<HashMap<String, OwnedVariant>>>, MenuError> {
        let proxy = GtkMenusProxy::builder(&self.connection)
            .destination(bus_name)
            .path(object_path)
            .build()
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))?;

        let mut menus_map = HashMap::new();
        let mut subscribed_groups = HashSet::new();
        let mut pending_groups = vec![0u32];

        while !pending_groups.is_empty() {
            for &g in &pending_groups {
                subscribed_groups.insert(g);
            }

            let chunks = proxy
                .start(pending_groups.clone())
                .await
                .map_err(|e| MenuError::Transport(e.to_string()))?;
            pending_groups.clear();

            for (group_id, menu_id, items) in chunks {
                for item in &items {
                    for key in [":section", ":submenu"] {
                        if let Some((ref_group, _)) = GtkMenuParser::extract_link_tuple(item, key)
                            && !subscribed_groups.contains(&ref_group)
                            && !pending_groups.contains(&ref_group)
                        {
                            pending_groups.push(ref_group);
                        }
                    }
                }
                menus_map.insert((group_id, menu_id), items);
            }
        }

        let all_groups: Vec<u32> = subscribed_groups.into_iter().collect();
        let _ = proxy.end(all_groups).await;
        Ok(menus_map)
    }

    pub async fn activate_custom_gtk_action(
        &self,
        info: &ActiveWindowInfo,
        hooked_bus_name: Option<&str>,
        action_spec: &str,
    ) -> bool {
        activate_custom_gtk_action(&self.connection, info, hooked_bus_name, action_spec).await
    }

    /// Queries `org.genubar.Gtk4Bridge.DumpIntrospection` on a hooked GTK4 process.
    pub async fn dump_gtk4_introspection(&self, bus_name: &str) -> Result<String, MenuError> {
        dump_gtk4_introspection(&self.connection, bus_name).await
    }
}

#[async_trait]
impl MenuProvider for GtkMenuClient {
    async fn fetch_menu(&self, target: &MenuTarget) -> Result<MenuNode, MenuError> {
        let MenuTarget::Gtk {
            bus_name,
            menubar_path,
            app_menu_path,
            app_path,
            win_path,
        } = target
        else {
            return Err(MenuError::Transport(
                "GtkMenuClient requires a Gtk target".into(),
            ));
        };

        let mut all_states = HashMap::new();
        let is_gtk4_hook = menubar_path.as_deref() == Some(genubar_protocol::GTK4_MENU_PATH);

        let zoom_pct = if is_gtk4_hook {
            let (hook_states, z_pct) = tokio::join!(
                fetch_hook_action_states(&self.connection, bus_name, menubar_path.as_deref()),
                query_gtk4_zoom_percent(&self.connection, bus_name, menubar_path.as_deref())
            );
            all_states.extend(hook_states);
            z_pct
        } else {
            let (app_s, win_s, unity_s) = tokio::join!(
                fetch_action_states(&self.connection, bus_name, "app", app_path.as_deref()),
                fetch_action_states(&self.connection, bus_name, "win", win_path.as_deref()),
                fetch_action_states(&self.connection, bus_name, "unity", menubar_path.as_deref())
            );
            all_states.extend(app_s);
            all_states.extend(win_s);
            all_states.extend(unity_s);
            100
        };

        let mut next_id = 1;
        let mut local_cache = HashMap::new();
        let mut top_children = Vec::new();

        if let Some(app_menu_p) = app_menu_path.as_deref().filter(|p| !p.is_empty())
            && let Ok(groups) = self.fetch_all_menu_groups(bus_name, app_menu_p).await
        {
            let mut visited = HashSet::new();
            let app_items = GtkMenuParser::build_nodes_from_group(
                (0, 0),
                &groups,
                &all_states,
                zoom_pct,
                &mut next_id,
                &mut local_cache,
                &mut visited,
            );
            if !app_items.is_empty() {
                let id = next_id;
                next_id += 1;
                top_children.push(MenuNode {
                    id,
                    label: String::from("Application"),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    origin_path: None,
                    children: app_items,
                    icon: None,
                });
            }
        }

        if let Some(menubar_p) = menubar_path.as_deref().filter(|p| !p.is_empty()) {
            let groups = self.fetch_all_menu_groups(bus_name, menubar_p).await?;
            let mut visited = HashSet::new();
            let bar_items = GtkMenuParser::build_nodes_from_group(
                (0, 0),
                &groups,
                &all_states,
                zoom_pct,
                &mut next_id,
                &mut local_cache,
                &mut visited,
            );
            top_children.extend(bar_items);
        }

        if top_children.is_empty() {
            return Err(MenuError::NotFound(format!(
                "No GTK menu items found on {bus_name}"
            )));
        }

        let scope = Self::target_cache_scope(menubar_path.as_deref(), win_path.as_deref());
        {
            let mut cache = self.action_cache.write().await;
            cache.retain(|(b, s, _), _| b != bus_name || s != &scope);
            for (k, v) in local_cache {
                cache.insert((bus_name.clone(), scope.clone(), k), v);
            }
        }

        Ok(MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            origin_path: None,
            children: top_children,
            icon: None,
        })
    }

    async fn fetch_submenu(
        &self,
        _target: &MenuTarget,
        submenu_id: i32,
    ) -> Result<MenuNode, MenuError> {
        Err(MenuError::NotFound(format!(
            "On-demand submenu fetch not needed for GTK (id={submenu_id})"
        )))
    }

    async fn activate_item(&self, target: &MenuTarget, item_id: i32) -> Result<(), MenuError> {
        let MenuTarget::Gtk {
            bus_name,
            menubar_path,
            app_path,
            win_path,
            ..
        } = target
        else {
            return Err(MenuError::Transport(
                "GtkMenuClient requires a Gtk target".into(),
            ));
        };

        let scope = Self::target_cache_scope(menubar_path.as_deref(), win_path.as_deref());
        let cache_key = (bus_name.clone(), scope, item_id);
        if !self.action_cache.read().await.contains_key(&cache_key) {
            let _ = self.fetch_menu(target).await?;
        }

        let cached = self
            .action_cache
            .read()
            .await
            .get(&cache_key)
            .cloned()
            .ok_or_else(|| MenuError::NotFound(format!("GTK menu item ID {item_id} not found")))?;

        let params: Vec<OwnedVariant> = cached.target_param.into_iter().collect();

        if menubar_path.as_deref() == Some(genubar_protocol::GTK4_MENU_PATH)
            && try_activate_bridge_action(
                &self.connection,
                bus_name,
                &cached.full_action,
                params.clone(),
            )
            .await
        {
            return Ok(());
        }

        let (prefix, short_action) = cached
            .full_action
            .split_once('.')
            .unwrap_or(("win", cached.full_action.as_str()));

        let action_path = match prefix {
            "app" => app_path.as_deref(),
            "win" => win_path.as_deref(),
            "unity" => menubar_path.as_deref(),
            _ => win_path.as_deref().or(app_path.as_deref()),
        }
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            MenuError::NotFound(format!(
                "Missing object path for action prefix '{prefix}' (action: {})",
                cached.full_action
            ))
        })?;

        activate_org_gtk_action(
            &self.connection,
            bus_name,
            action_path,
            short_action,
            params,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_cache_scope_separates_concurrent_windows_on_same_bus() {
        let s1 = GtkMenuClient::target_cache_scope(
            Some("/org/example/App/menus/menubar"),
            Some("/org/example/App/window/1"),
        );
        let s2 = GtkMenuClient::target_cache_scope(
            Some("/org/example/App/menus/menubar"),
            Some("/org/example/App/window/2"),
        );
        assert_ne!(s1, s2);
    }
}
