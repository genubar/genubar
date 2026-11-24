use std::collections::HashMap;
use zbus::proxy;
use zbus::{OwnedVariant, Signature, Variant};

#[proxy(interface = "org.gtk.Menus")]
pub trait GtkMenus {
    fn start(
        &self,
        groups: Vec<u32>,
    ) -> zbus::Result<Vec<(u32, u32, Vec<HashMap<String, OwnedVariant>>)>>;

    fn end(&self, groups: Vec<u32>) -> zbus::Result<()>;
}

#[proxy(interface = "org.gtk.Actions")]
pub trait GtkActions {
    fn describe_all(&self) -> zbus::Result<HashMap<String, (bool, Signature, Vec<OwnedVariant>)>>;

    fn activate(
        &self,
        action_name: &str,
        parameter: Vec<OwnedVariant>,
        platform_data: HashMap<&str, Variant<'_>>,
    ) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.genubar.Gtk4Bridge",
    default_path = "/org/genubar/gtk4_bridge"
)]
pub trait Gtk4Bridge {
    fn activate(&self, action: &str, param: Vec<OwnedVariant>) -> zbus::Result<()>;

    fn get_zoom_percent(&self) -> zbus::Result<i32>;

    fn get_action_states(&self) -> zbus::Result<Vec<(String, bool, String, String)>>;

    fn refresh(&self) -> zbus::Result<()>;

    fn dump_introspection(&self) -> zbus::Result<String>;
}
