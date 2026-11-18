#[derive(Debug, Clone)]
pub enum AtspiActionRef {
    Direct {
        bus_name: String,
        object_path: String,
    },
    CachedReplay {
        bus_name: String,
        menubar_path: String,
        label_path: Vec<String>,
    },
}
