use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub version: String,
    pub author: String,
    pub description: String,
}
