use crate::models::ConflictReport;
use genubar_core::MenuError;
use genubar_platform::data_dir;
use genubar_platform::fs_utils::write_text_file;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseOutcome {
    Released(Option<String>),
    StillOwnedByOthers,
    MissingFromLedger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub resource_key: String,
    pub original_value: Option<String>,
    pub applied_value: String,
    pub owners: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchLedger {
    pub applied_patches: HashSet<String>,
    pub resources: HashMap<String, ResourceRecord>,
    #[serde(default)]
    pub notified_apps: HashSet<String>,
    #[serde(skip)]
    pub custom_path: Option<PathBuf>,
}

impl PatchLedger {
    pub fn ledger_path() -> PathBuf {
        data_dir().join("patch_ledger.json")
    }

    pub fn load_or_default() -> Self {
        Self::load_or_default_from(None)
    }

    /// Loads or initializes a ledger backed by a custom file path (used by isolated tests and custom tools).
    pub fn load_or_default_at(path: PathBuf) -> Self {
        Self::load_or_default_from(Some(path))
    }

    fn load_or_default_from(custom_path: Option<PathBuf>) -> Self {
        let path = custom_path.clone().unwrap_or_else(Self::ledger_path);
        let mut ledger = if !path.exists() {
            Self::default()
        } else {
            match fs::read_to_string(&path) {
                Ok(raw) => serde_json::from_str::<PatchLedger>(&raw).unwrap_or_else(|err| {
                    tracing::warn!(
                        error = %err,
                        path = %path.display(),
                        "Corrupted patch ledger JSON, resetting ledger"
                    );
                    Self::default()
                }),
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        path = %path.display(),
                        "Could not read patch ledger, using empty default"
                    );
                    Self::default()
                }
            }
        };
        ledger.custom_path = custom_path;
        ledger
    }

    pub fn save(&self) -> Result<(), MenuError> {
        let path = self.custom_path.clone().unwrap_or_else(Self::ledger_path);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| MenuError::Config(format!("Failed to serialize patch ledger: {e}")))?;
        write_text_file(&path, &json)
    }

    pub fn check_conflict(
        &self,
        patch_id: &str,
        resource_key: &str,
        new_value: &str,
    ) -> Option<ConflictReport> {
        let existing = self.resources.get(resource_key)?;
        if existing.applied_value == new_value {
            return None;
        }
        let other_owners: Vec<String> = existing
            .owners
            .iter()
            .filter(|id| *id != patch_id)
            .cloned()
            .collect();

        if other_owners.is_empty() {
            None
        } else {
            Some(ConflictReport {
                resource_key: resource_key.to_string(),
                existing_value: existing.applied_value.clone(),
                new_value: new_value.to_string(),
                conflicting_owners: other_owners,
            })
        }
    }

    pub fn record_acquire(
        &mut self,
        patch_id: &str,
        resource_key: String,
        original_system_value: Option<String>,
        new_value: String,
        overwrite_owners: bool,
    ) {
        let entry = self
            .resources
            .entry(resource_key.clone())
            .or_insert_with(|| ResourceRecord {
                resource_key,
                original_value: original_system_value,
                applied_value: new_value.clone(),
                owners: Vec::new(),
            });

        entry.applied_value = new_value;
        if overwrite_owners {
            entry.owners.clear();
        }
        if !entry.owners.contains(&patch_id.to_string()) {
            entry.owners.push(patch_id.to_string());
        }
    }

    pub fn record_release(&mut self, patch_id: &str, resource_key: &str) -> ReleaseOutcome {
        let Some(rec) = self.resources.get_mut(resource_key) else {
            return ReleaseOutcome::MissingFromLedger;
        };

        let was_owner = rec.owners.iter().any(|id| id == patch_id);
        rec.owners.retain(|id| id != patch_id);

        if rec.owners.is_empty() {
            let orig = rec.original_value.clone();
            self.resources.remove(resource_key);
            if was_owner {
                ReleaseOutcome::Released(orig)
            } else {
                ReleaseOutcome::MissingFromLedger
            }
        } else {
            ReleaseOutcome::StillOwnedByOthers
        }
    }
}
