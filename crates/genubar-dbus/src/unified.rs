use crate::introspect::DbusIntrospector;
use crate::protocols::atspi::AtspiMenuClient;
use crate::protocols::canonical::CanonicalMenuClient;
use crate::protocols::gtk::GtkMenuClient;
use async_trait::async_trait;
use genubar_core::errors::MenuError;
use genubar_core::{MenuNode, MenuProvider, MenuTarget};
use zbus::Connection;

#[derive(Clone)]
pub struct UnifiedMenuService {
    connection: Connection,
    canonical: CanonicalMenuClient,
    gtk: GtkMenuClient,
    atspi: AtspiMenuClient,
}

impl UnifiedMenuService {
    pub async fn new() -> Result<Self, MenuError> {
        let connection = Connection::session()
            .await
            .map_err(|e| MenuError::Transport(e.to_string()))?;

        Ok(Self {
            canonical: CanonicalMenuClient::new(connection.clone()),
            gtk: GtkMenuClient::new(connection.clone()),
            atspi: AtspiMenuClient::new(connection.clone()),
            connection,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn canonical(&self) -> &CanonicalMenuClient {
        &self.canonical
    }

    pub fn atspi(&self) -> &AtspiMenuClient {
        &self.atspi
    }

    pub fn gtk(&self) -> &GtkMenuClient {
        &self.gtk
    }

    pub async fn discover_paths(&self, bus_name: &str) -> Result<Vec<(String, String)>, MenuError> {
        DbusIntrospector::discover_paths(&self.connection, bus_name).await
    }

    pub async fn discover_dbus_target_by_pid(&self, target_pid: u32) -> Option<MenuTarget> {
        DbusIntrospector::discover_dbus_target_by_pid(&self.connection, target_pid).await
    }
}

#[async_trait]
impl MenuProvider for UnifiedMenuService {
    async fn fetch_menu(&self, target: &MenuTarget) -> Result<MenuNode, MenuError> {
        match target {
            MenuTarget::Canonical { .. } => self.canonical.fetch_menu(target).await,
            MenuTarget::Gtk { .. } => self.gtk.fetch_menu(target).await,
            MenuTarget::Atspi { .. } => self.atspi.fetch_menu(target).await,
        }
    }

    async fn fetch_submenu(
        &self,
        target: &MenuTarget,
        item_id: i32,
    ) -> Result<MenuNode, MenuError> {
        match target {
            MenuTarget::Canonical { .. } => self.canonical.fetch_submenu(target, item_id).await,
            MenuTarget::Gtk { .. } => self.gtk.fetch_submenu(target, item_id).await,
            MenuTarget::Atspi { .. } => self.atspi.fetch_submenu(target, item_id).await,
        }
    }

    async fn activate_item(&self, target: &MenuTarget, item_id: i32) -> Result<(), MenuError> {
        match target {
            MenuTarget::Canonical { .. } => self.canonical.activate_item(target, item_id).await,
            MenuTarget::Gtk { .. } => self.gtk.activate_item(target, item_id).await,
            MenuTarget::Atspi { .. } => self.atspi.activate_item(target, item_id).await,
        }
    }
}
