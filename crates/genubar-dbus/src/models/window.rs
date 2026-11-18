#[derive(Debug, Clone)]
pub struct RegisteredWindow {
    pub window_id: u32,
    pub pid: Option<u32>,
    pub bus_name: String,
    pub object_path: String,
}
