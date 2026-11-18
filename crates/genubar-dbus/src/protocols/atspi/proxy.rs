use zbus::OwnedObjectPath;
use zbus::proxy;

#[proxy(
    interface = "org.a11y.Bus",
    default_service = "org.a11y.Bus",
    default_path = "/org/a11y/bus"
)]
pub trait A11yBus {
    fn get_address(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.a11y.Status",
    default_service = "org.a11y.Bus",
    default_path = "/org/a11y/bus"
)]
pub trait A11yStatus {
    #[zbus(property)]
    fn set_is_enabled(&self, value: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_screen_reader_enabled(&self, value: bool) -> zbus::Result<()>;
}

#[proxy(interface = "org.a11y.atspi.Accessible")]
pub trait AtspiAccessible {
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;

    fn get_role_name(&self) -> zbus::Result<String>;

    fn get_state(&self) -> zbus::Result<Vec<u32>>;

    fn get_children(&self) -> zbus::Result<Vec<(String, OwnedObjectPath)>>;
}

#[proxy(interface = "org.a11y.atspi.Action")]
pub trait AtspiAction {
    fn n_actions(&self) -> zbus::Result<i32>;

    fn get_name(&self, index: i32) -> zbus::Result<String>;

    fn do_action(&self, index: i32) -> zbus::Result<bool>;

    fn get_key_binding(&self, index: i32) -> zbus::Result<String>;
}