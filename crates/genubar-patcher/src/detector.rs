use crate::models::{AppDetectionRule, PatchManifest};
use genubar_core::ActiveWindowInfo;
use genubar_platform::desktop_probe::{any_binary_in_path, any_flatpak_app, has_any_desktop_file};
use genubar_platform::home_dir;
use std::fs;
use std::path::Path;

pub struct SystemDetector;

impl SystemDetector {
    pub fn is_app_installed(rule: &AppDetectionRule) -> bool {
        let home = home_dir();
        any_binary_in_path(&rule.binaries)
            || has_any_desktop_file(&rule.desktop_files)
            || any_flatpak_app(&rule.flatpak_ids)
            || Self::has_home_dir_match(&home, &rule.home_dirs)
    }

    pub fn matches_focused_window(manifest: &PatchManifest, info: &ActiveWindowInfo) -> bool {
        let app_id_low = info.app_id.to_lowercase();
        let title_low = info.title.to_lowercase();
        manifest.detection.window_matches.iter().any(|pattern| {
            let pat = pattern.to_lowercase();
            !pat.is_empty() && (app_id_low.contains(&pat) || title_low.contains(&pat))
        })
    }

    fn has_home_dir_match(home: &Path, home_dirs: &[String]) -> bool {
        for prefix in home_dirs {
            let full = home.join(prefix);
            if full.exists() {
                return true;
            }
            if let (Some(parent), Some(file_prefix)) =
                (full.parent(), full.file_name().and_then(|n| n.to_str()))
                && let Ok(entries) = fs::read_dir(parent) {
                    let matched = entries.flatten().any(|entry| {
                        entry
                            .file_name()
                            .to_str()
                            .map(|s| s.starts_with(file_prefix))
                            .unwrap_or(false)
                    });
                    if matched {
                        return true;
                    }
                }
        }
        false
    }
}