use genubar_core::MenuTarget;
use genubar_core::errors::MenuError;
use genubar_platform::proc_utils::pids_match;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::{Connection, proxy};

#[proxy(interface = "org.freedesktop.DBus.Introspectable")]
trait Introspectable {
    fn introspect(&self) -> zbus::Result<String>;
}

pub struct DbusIntrospector;

impl DbusIntrospector {
    pub async fn discover_paths(
        connection: &Connection,
        bus_name: &str,
    ) -> Result<Vec<(String, String)>, MenuError> {
        let mut found = Vec::new();
        let mut queue = vec![String::from("/")];

        while let Some(current_path) = queue.pop() {
            let proxy = IntrospectableProxy::builder(connection)
                .destination(bus_name)
                .path(current_path.as_str())
                .build()
                .await
                .map_err(|e| MenuError::Transport(e.to_string()))?;

            let Ok(xml) = proxy.introspect().await else {
                continue;
            };

            for iface in [
                "com.canonical.dbusmenu",
                "org.gtk.Menus",
                "org.gtk.Actions",
            ] {
                if xml.contains(iface) {
                    found.push((current_path.clone(), iface.to_string()));
                }
            }

            for child_name in Self::extract_child_node_names(&xml) {
                let next_path = if current_path == "/" {
                    format!("/{child_name}")
                } else {
                    format!("{current_path}/{child_name}")
                };
                queue.push(next_path);
            }
        }

        Ok(found)
    }

    fn extract_child_node_names(xml: &str) -> Vec<&str> {
        let mut out = Vec::new();
        for line in xml.lines() {
            if let Some(rest) = line.trim().strip_prefix("<node name=\"")
                && let Some(end_idx) = rest.find('"') {
                    let child_name = &rest[..end_idx];
                    if !child_name.is_empty() && !child_name.starts_with('/') {
                        out.push(child_name);
                    }
                }
        }
        out
    }

    pub async fn discover_dbus_target_by_pid(
        connection: &Connection,
        target_pid: u32,
    ) -> Option<MenuTarget> {
        if target_pid == 0 {
            return None;
        }
        let dbus_proxy = DBusProxy::new(connection).await.ok()?;
        let names = dbus_proxy.list_names().await.ok()?;

        for name in names {
            let name_str = name.as_str();
            if !name_str.starts_with(':') {
                continue;
            }
            let Ok(bus_name) = BusName::try_from(name_str) else {
                continue;
            };
            let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus_name).await else {
                continue;
            };
            if pids_match(target_pid, pid)
                && let Some(target) = Self::inspect_peer_paths(connection, name_str).await {
                    return Some(target);
                }
        }
        None
    }

    async fn inspect_peer_paths(connection: &Connection, bus_name: &str) -> Option<MenuTarget> {
        let paths = Self::discover_paths(connection, bus_name).await.ok()?;

        if let Some((path, _)) = paths
            .iter()
            .find(|(_, iface)| iface == "com.canonical.dbusmenu")
        {
            return Some(MenuTarget::Canonical {
                bus_name: bus_name.to_string(),
                object_path: path.clone(),
            });
        }

        let menubar_path = paths
            .iter()
            .find(|(_, iface)| iface == "org.gtk.Menus")
            .map(|(p, _)| p.clone())?;

        let action_paths: Vec<String> = paths
            .iter()
            .filter(|(_, iface)| iface == "org.gtk.Actions")
            .map(|(p, _)| p.clone())
            .collect();

        let app_path = action_paths
            .iter()
            .find(|p| !p.contains("/window/"))
            .cloned();
        let win_path = action_paths
            .iter()
            .find(|p| p.contains("/window/") || p.ends_with("gtk4_win"))
            .cloned();

        Some(MenuTarget::Gtk {
            bus_name: bus_name.to_string(),
            menubar_path: Some(menubar_path),
            app_menu_path: None,
            app_path,
            win_path,
        })
    }
}
