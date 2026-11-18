use zbus::OwnedVariant;

#[derive(Debug, Clone)]
pub struct ActionState {
    pub enabled: bool,
    pub state: Option<OwnedVariant>,
    pub printed_state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CachedGtkAction {
    pub full_action: String,
    pub target_param: Option<OwnedVariant>,
}
