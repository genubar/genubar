use super::StatefulStepApplier;
use crate::utils::paths::expand_path;
use genubar_core::MenuError;
use genubar_platform::fs_utils::{read_text_file, write_text_file};
use std::path::Path;

pub(crate) struct JsonSetApplier<'a> {
    pub file: &'a Path,
    pub key: &'a str,
    pub value: &'a serde_json::Value,
}

impl StatefulStepApplier for JsonSetApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("json:{}:{}", self.file.display(), self.key),
            self.value.to_string(),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(self.file, self.key, self.value)
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.file, self.key, orig_opt)
    }
}

pub fn apply(
    file: &Path,
    key: &str,
    value: &serde_json::Value,
) -> Result<Option<String>, MenuError> {
    let path = expand_path(file);
    let mut root: serde_json::Value = if path.is_file() {
        let raw = read_text_file(&path)?;
        if raw.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&raw).map_err(|e| {
                MenuError::ParseError(format!("Invalid JSON in {}: {e}", path.display()))
            })?
        }
    } else {
        serde_json::json!({})
    };

    let obj = root.as_object_mut().ok_or_else(|| {
        MenuError::ParseError(format!("Root of {} is not a JSON object", path.display()))
    })?;

    let prev_val_str = obj.get(key).and_then(|v| serde_json::to_string(v).ok());

    obj.insert(key.to_string(), value.clone());

    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|e| MenuError::ParseError(format!("JSON serialization error: {e}")))?;
    write_text_file(&path, &(formatted + "\n"))?;

    Ok(prev_val_str)
}

pub fn revert(file: &Path, key: &str, orig_opt: Option<String>) -> Result<(), MenuError> {
    let path = expand_path(file);
    if !path.is_file() {
        return Ok(());
    }

    let raw = read_text_file(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MenuError::ParseError(format!("Invalid JSON in {}: {e}", path.display())))?;

    if let Some(obj) = root.as_object_mut() {
        match orig_opt.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
            Some(old_json_val) => {
                obj.insert(key.to_string(), old_json_val);
            }
            None => {
                obj.remove(key);
            }
        }
        let formatted = serde_json::to_string_pretty(&root)
            .map_err(|e| MenuError::ParseError(e.to_string()))?;
        write_text_file(&path, &(formatted + "\n"))?;
    }
    Ok(())
}
