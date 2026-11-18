use genubar_core::MenuError;
use genubar_platform::fs_utils::{read_text_file as read_string, write_text_file as write_string};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;

pub fn read_toml_file<T: DeserializeOwned>(path: &Path) -> Result<T, MenuError> {
    let raw = read_string(path)?;
    toml::from_str::<T>(&raw)
        .map_err(|e| MenuError::Config(format!("TOML parse error in {}: {e}", path.display())))
}

pub fn write_toml_file<T: Serialize>(path: &Path, value: &T) -> Result<(), MenuError> {
    let serialized = toml::to_string_pretty(value).map_err(|e| {
        MenuError::Config(format!(
            "TOML serialization error for {}: {e}",
            path.display()
        ))
    })?;
    write_string(path, &serialized)
}

pub fn parse_embedded_toml<T: DeserializeOwned>(
    raw_toml: &str,
    context_name: &str,
) -> Result<T, MenuError> {
    toml::from_str::<T>(raw_toml)
        .map_err(|e| MenuError::Config(format!("Invalid embedded default {context_name}: {e}")))
}