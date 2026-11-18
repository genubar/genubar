use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    Abort,
    Overwrite,
    SkipConflictingSteps,
    DisableConflictingPatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    pub resource_key: String,
    pub existing_value: String,
    pub new_value: String,
    pub conflicting_owners: Vec<String>,
}