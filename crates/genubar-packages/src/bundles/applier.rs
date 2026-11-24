use super::{BundleManager, BundleOrchestrator};
use genubar_config::{BundleManifest, ConfigManager, IconPackRepo};
use genubar_core::MenuError;
use genubar_patcher::{ConflictStrategy, PatchManager};

#[derive(Debug, Clone)]
pub struct BundleActionOutcome {
    pub bundle_id: String,
    pub enabled: bool,
    pub changed_patches: Vec<String>,
    pub post_hints: Vec<String>,
}

// Toggles profiles, icon packs, active style, and patches for a bundle in one coordinated flow.
pub fn set_bundle_enabled(
    cfg: &mut ConfigManager,
    patcher: &mut PatchManager,
    bundle_id: &str,
    enabled: bool,
) -> Result<BundleActionOutcome, MenuError> {
    let bundle_mgr = BundleManager::new(cfg.base_dir().to_path_buf());
    let Some(bundle_view) = bundle_mgr
        .list_bundles()
        .into_iter()
        .find(|b| b.manifest.id == bundle_id)
    else {
        return Err(MenuError::NotFound(format!(
            "Bundle '{bundle_id}' not found"
        )));
    };

    if enabled && !bundle_view.is_compatible {
        return Err(MenuError::Config(format!(
            "Bundle '{}' requires Genubar v{} (update Genubar first)",
            bundle_view.manifest.name,
            bundle_view
                .manifest
                .min_app_version
                .as_deref()
                .unwrap_or("newer")
        )));
    }

    toggle_bundle_components(cfg, &bundle_view.manifest, enabled)?;

    let patch_ids: Vec<(String, bool)> =
        BundleOrchestrator::matching_patches(patcher, bundle_id, &bundle_view.manifest.patches)
            .into_iter()
            .map(|m| (m.meta.id.clone(), m.is_fully_compatible()))
            .collect();

    let policy = cfg.settings.general.patch_error_policy;
    let mut changed_patches = Vec::new();
    let mut post_hints = Vec::new();

    for (pid, is_compat) in patch_ids {
        let currently_applied = patcher.ledger.applied_patches.contains(&pid);
        if enabled {
            if !is_compat || currently_applied {
                continue;
            }
            if let Some(hint) =
                patcher.apply_patch_ext(&pid, ConflictStrategy::Overwrite, policy, true)?
            {
                post_hints.push(hint);
            }
            changed_patches.push(pid);
        } else if currently_applied {
            patcher.revert_patch_ext(&pid, true)?;
            changed_patches.push(pid);
        }
    }

    Ok(BundleActionOutcome {
        bundle_id: bundle_id.to_string(),
        enabled,
        changed_patches,
        post_hints,
    })
}

fn toggle_bundle_components(
    cfg: &mut ConfigManager,
    manifest: &BundleManifest,
    enabled: bool,
) -> Result<(), MenuError> {
    let profiles_dir = cfg.base_dir().join("profiles");

    for prof_file in &manifest.profiles {
        let stem = prof_file.trim_end_matches(".toml");
        if let Some(profile) = cfg.profiles.iter_mut().find(|p| {
            p.id == stem
                || p.file_path.file_name().and_then(|n| n.to_str()) == Some(prof_file.as_str())
                || format!("{}.toml", p.id) == *prof_file
        }) {
            profile.enabled = enabled;
            let dest = if !profile.file_path.as_os_str().is_empty() {
                profile.file_path.clone()
            } else {
                profiles_dir.join(prof_file)
            };
            if let Err(err) = genubar_config::utils::toml_io::write_toml_file(&dest, profile) {
                tracing::warn!(path = %dest.display(), error = %err, "Failed saving bundle profile");
            }
        }
    }

    for pack_id in &manifest.icon_packs {
        if let Some(pack) = cfg.icon_packs.iter_mut().find(|p| p.id == *pack_id) {
            pack.enabled = enabled;
            if let Err(err) = IconPackRepo::save(pack) {
                tracing::warn!(pack_id = %pack.id, error = %err, "Failed saving bundle icon pack");
            }
        }
    }

    if enabled {
        if let Some(first_style) = manifest.styles.first() {
            cfg.settings.style.active_style = first_style.clone();
            cfg.save_settings()?;
            cfg.sync_stylesheet();
        }
    } else if manifest
        .styles
        .iter()
        .any(|s| s == &cfg.settings.style.active_style)
        && cfg.settings.style.active_style != "default.css"
    {
        cfg.settings.style.active_style = String::from("default.css");
        cfg.save_settings()?;
        cfg.sync_stylesheet();
    }

    Ok(())
}
