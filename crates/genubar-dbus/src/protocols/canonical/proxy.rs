use std::collections::HashMap;
use zbus::proxy;
use zbus::{OwnedVariant, Variant};

#[proxy(interface = "com.canonical.dbusmenu", default_path = "/MenuBar")]
pub trait CanonicalDbusMenu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<&str>,
    ) -> zbus::Result<(u32, (i32, HashMap<String, OwnedVariant>, Vec<OwnedVariant>))>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;

    fn event(
        &self,
        id: i32,
        event_id: &str,
        data: &Variant<'_>,
        timestamp: u32,
    ) -> zbus::Result<()>;
}
