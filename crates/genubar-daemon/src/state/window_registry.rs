use genubar_core::MenuTarget;
use genubar_dbus::RegisteredWindow;
use genubar_platform::is_pid_gtk4_hooked;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracks live Canonical (`com.canonical.AppMenu.Registrar`) and hooked GTK4 windows.
#[derive(Default, Clone)]
pub struct WindowRegistry {
    by_wid: Arc<RwLock<HashMap<u32, RegisteredWindow>>>,
    by_pid: Arc<RwLock<HashMap<u32, RegisteredWindow>>>,
    last_good_canonical_target: Arc<RwLock<HashMap<u32, MenuTarget>>>,
    gtk4_by_pid: Arc<RwLock<HashMap<u32, MenuTarget>>>,
    gtk4_by_bus: Arc<RwLock<HashMap<String, MenuTarget>>>,
}

impl WindowRegistry {
    pub fn is_process_gtk4_hooked(pid: u32) -> bool {
        is_pid_gtk4_hooked(pid)
    }

    pub async fn insert_canonical(&self, entry: RegisteredWindow) {
        let target = MenuTarget::Canonical {
            bus_name: entry.bus_name.clone(),
            object_path: entry.object_path.clone(),
        };
        if let Some(pid) = entry.pid {
            self.by_pid.write().await.insert(pid, entry.clone());
            self.last_good_canonical_target
                .write()
                .await
                .insert(pid, target);
        }
        self.by_wid.write().await.insert(entry.window_id, entry);
    }

    pub async fn remove_canonical(&self, window_id: u32) -> Option<RegisteredWindow> {
        let removed = self.by_wid.write().await.remove(&window_id);
        if let Some(ref entry) = removed
            && let Some(pid) = entry.pid
        {
            self.by_pid.write().await.remove(&pid);
            let mut last_map = self.last_good_canonical_target.write().await;
            if let Some(MenuTarget::Canonical {
                bus_name,
                object_path,
            }) = last_map.get(&pid)
                && bus_name == &entry.bus_name
                && object_path == &entry.object_path
            {
                last_map.remove(&pid);
            }
        }
        removed
    }

    pub async fn invalidate_canonical_pid(&self, pid: u32) {
        if pid == 0 {
            return;
        }
        self.by_pid.write().await.remove(&pid);
        self.last_good_canonical_target.write().await.remove(&pid);
    }

    pub async fn resolve_canonical(&self, xid: u32, pid: u32) -> Option<MenuTarget> {
        if xid != 0
            && let Some(entry) = self.by_wid.read().await.get(&xid)
        {
            return Some(MenuTarget::Canonical {
                bus_name: entry.bus_name.clone(),
                object_path: entry.object_path.clone(),
            });
        }
        if pid != 0 {
            if let Some(entry) = self.by_pid.read().await.get(&pid) {
                return Some(MenuTarget::Canonical {
                    bus_name: entry.bus_name.clone(),
                    object_path: entry.object_path.clone(),
                });
            }
            if let Some(last_target) = self.last_good_canonical_target.read().await.get(&pid) {
                return Some(last_target.clone());
            }
        }
        None
    }

    pub async fn insert_gtk4(&self, pid: u32, target: MenuTarget) {
        if let MenuTarget::Gtk { ref bus_name, .. } = target
            && !bus_name.is_empty()
        {
            self.gtk4_by_bus
                .write()
                .await
                .insert(bus_name.clone(), target.clone());
        }
        if pid != 0 {
            self.gtk4_by_pid.write().await.insert(pid, target);
        }
    }

    pub async fn resolve_gtk4(&self, pid: u32, gtk_bus_name: &str) -> Option<MenuTarget> {
        if pid != 0
            && let Some(target) = self.gtk4_by_pid.read().await.get(&pid).cloned()
        {
            return Some(target);
        }
        if !gtk_bus_name.is_empty()
            && let Some(target) = self.gtk4_by_bus.read().await.get(gtk_bus_name).cloned()
        {
            return Some(target);
        }
        None
    }
}
