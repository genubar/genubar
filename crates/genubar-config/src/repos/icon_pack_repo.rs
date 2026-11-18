use crate::models::icons::IconPackManifest;
use crate::utils::toml_io::{read_toml_file, write_toml_file};
use genubar_core::MenuError;
use std::fs;
use std::path::Path;

pub struct IconPackRepo;

impl IconPackRepo {
    pub fn load_all(icons_dir: &Path) -> Vec<IconPackManifest> {
        let mut packs = Vec::new();
        let Ok(entries) = fs::read_dir(icons_dir) else {
            return packs;
        };

        for entry in entries.flatten() {
            let dir_path = entry.path();
            if dir_path.is_dir() {
                let pack_toml = dir_path.join("pack.toml");
                if pack_toml.is_file() {
                    match read_toml_file::<IconPackManifest>(&pack_toml) {
                        Ok(mut pack) => {
                            pack.pack_dir = dir_path;
                            packs.push(pack);
                        }
                        Err(err) => tracing::warn!(error = %err, "Failed to read icon pack"),
                    }
                }
            }
        }

        packs.sort_by_key(|p| p.priority);
        packs
    }

    pub fn save(pack: &IconPackManifest) -> Result<(), MenuError> {
        write_toml_file(&pack.pack_dir.join("pack.toml"), pack)
    }
}
