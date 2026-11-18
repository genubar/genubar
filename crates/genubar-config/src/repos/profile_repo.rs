use crate::models::profile::AppProfile;
use crate::utils::slug::sanitize_slug;
use crate::utils::toml_io::{read_toml_file, write_toml_file};
use genubar_core::MenuError;
use genubar_platform::fs_utils::remove_file_if_exists;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ProfileRepo;

impl ProfileRepo {
    pub fn load_all(profiles_dir: &Path) -> Vec<AppProfile> {
        let mut list = Vec::new();
        let Ok(entries) = fs::read_dir(profiles_dir) else {
            return list;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                match read_toml_file::<AppProfile>(&path) {
                    Ok(mut profile) => {
                        profile.file_path = path;
                        list.push(profile);
                    }
                    Err(err) => tracing::warn!(error = %err, "Failed to read profile"),
                }
            }
        }

        list.sort_by_key(|p| p.priority);
        list
    }

    pub fn save(
        profiles_dir: &Path,
        profiles: &mut Vec<AppProfile>,
        mut profile: AppProfile,
    ) -> Result<PathBuf, MenuError> {
        let out_path = if !profile.file_path.as_os_str().is_empty()
            && profile.file_path.starts_with(profiles_dir)
        {
            profile.file_path.clone()
        } else {
            let slug = sanitize_slug(&profile.id, &profile.name);
            profiles_dir.join(format!("{slug}.toml"))
        };

        profile.file_path = out_path.clone();
        write_toml_file(&out_path, &profile)?;

        profiles.retain(|p| p.id != profile.id);
        profiles.push(profile);
        profiles.sort_by_key(|p| p.priority);
        Ok(out_path)
    }

    pub fn delete(
        profiles_dir: &Path,
        profiles: &mut Vec<AppProfile>,
        profile_id: &str,
    ) -> Result<(), MenuError> {
        let existing_path = profiles
            .iter()
            .find(|p| p.id == profile_id)
            .map(|p| p.file_path.clone())
            .filter(|p| !p.as_os_str().is_empty());

        let path = existing_path.unwrap_or_else(|| profiles_dir.join(format!("{profile_id}.toml")));
        remove_file_if_exists(&path)?;
        profiles.retain(|p| p.id != profile_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_repo_preserves_and_deletes_mismatched_filename_via_file_path() {
        let tmp_dir = std::env::temp_dir().join("genubar_test_profile_repo");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let disk_path = tmp_dir.join("loupe_papers.toml");
        let profile = AppProfile {
            id: String::from("org_gnome_Loupe"),
            name: String::from("Loupe Image Viewer"),
            ..Default::default()
        };
        write_toml_file(&disk_path, &profile).unwrap();

        let mut loaded = ProfileRepo::load_all(&tmp_dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].file_path, disk_path);

        let mut edited = loaded[0].clone();
        edited.priority = 42;
        let saved_path = ProfileRepo::save(&tmp_dir, &mut loaded, edited).unwrap();
        assert_eq!(saved_path, disk_path);
        assert_eq!(fs::read_dir(&tmp_dir).unwrap().count(), 1);

        ProfileRepo::delete(&tmp_dir, &mut loaded, "org_gnome_Loupe").unwrap();
        assert!(loaded.is_empty());
        assert!(!disk_path.exists());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
