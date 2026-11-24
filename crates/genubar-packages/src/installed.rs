use crate::bundles::BundleManager;
use crate::models::InstalledPackageItem;
use genubar_config::ConfigManager;
use genubar_core::{MenuError, PackageKind};
use genubar_patcher::{PackageOrigin, PatchManager};
use genubar_pipeline::PythonRunner;
use genubar_platform::fs_utils::{remove_dir_if_exists, remove_file_if_exists};
use std::fs;

// Collects all installed bundles, patches, profiles, styles, icon packs, and Python scripts.
pub fn collect_all_installed_items(
    cfg: &ConfigManager,
    patcher: &PatchManager,
) -> Vec<InstalledPackageItem> {
    let mut items = Vec::new();
    let bundle_mgr = BundleManager::new(cfg.base_dir().to_path_buf());

    for b in bundle_mgr.list_bundles() {
        items.push(InstalledPackageItem {
            id: b.manifest.id.clone(),
            name: b.manifest.name,
            version: b.manifest.version,
            author: b.manifest.author,
            kind: PackageKind::Bundle,
            path: Some(cfg.base_dir().join("bundles").join(&b.manifest.id)),
            can_delete: true,
        });
    }

    for p in &patcher.patches {
        let is_builtin = p.meta.origin == PackageOrigin::BuiltIn;
        let file_path = cfg
            .base_dir()
            .join("patches")
            .join(format!("{}.toml", p.meta.id));
        let has_file = file_path.exists();
        items.push(InstalledPackageItem {
            id: p.meta.id.clone(),
            name: p.meta.name.clone(),
            version: p.meta.version.clone(),
            author: p.meta.author.clone(),
            kind: PackageKind::Patch,
            path: has_file.then_some(file_path),
            can_delete: !is_builtin || has_file,
        });
    }

    for pr in &cfg.profiles {
        let file_path = if !pr.file_path.as_os_str().is_empty() {
            pr.file_path.clone()
        } else {
            cfg.base_dir()
                .join("profiles")
                .join(format!("{}.toml", pr.id))
        };
        items.push(InstalledPackageItem {
            id: pr.id.clone(),
            name: pr.name.clone(),
            version: pr.version.clone(),
            author: pr.author.clone(),
            kind: PackageKind::Profile,
            path: Some(file_path),
            can_delete: true,
        });
    }

    for st in cfg.styles.list_styles() {
        let is_default = st.file_name == "default.css";
        items.push(InstalledPackageItem {
            id: st.file_name.clone(),
            name: st.name,
            version: st.version,
            author: st.author,
            kind: PackageKind::Style,
            path: Some(cfg.styles.styles_dir().join(&st.file_name)),
            can_delete: !is_default,
        });
    }

    for ic in &cfg.icon_packs {
        items.push(InstalledPackageItem {
            id: ic.id.clone(),
            name: ic.name.clone(),
            version: String::from("1.0.0"),
            author: if ic.author.is_empty() {
                String::from("local")
            } else {
                ic.author.clone()
            },
            kind: PackageKind::IconPack,
            path: Some(ic.pack_dir.clone()),
            can_delete: true,
        });
    }

    let scripts_dir = cfg.base_dir().join("scripts");
    if let Ok(entries) = fs::read_dir(scripts_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("py")
                && let Some(meta) = PythonRunner::parse_and_validate_header(&p)
            {
                items.push(InstalledPackageItem {
                    id: meta.id,
                    name: meta.name,
                    version: meta.version,
                    author: meta.author,
                    kind: PackageKind::Script,
                    path: Some(p),
                    can_delete: true,
                });
            }
        }
    }

    items
}

pub fn delete_installed_item(
    cfg: &mut ConfigManager,
    patcher: &mut PatchManager,
    item: &InstalledPackageItem,
) -> Result<(), MenuError> {
    if !item.can_delete {
        return Err(MenuError::Config(String::from(
            "Built-in default items cannot be deleted",
        )));
    }
    match item.kind {
        PackageKind::Bundle => {
            BundleManager::new(cfg.base_dir().to_path_buf()).uninstall_bundle(&item.id)?;
        }
        PackageKind::Patch => {
            if patcher.ledger.applied_patches.contains(&item.id) {
                let _ = patcher.revert_patch_ext(&item.id, true);
            }
            if let Some(ref p) = item.path {
                let _ = remove_file_if_exists(p);
            }
            patcher.patches.retain(|p| p.meta.id != item.id);
        }
        PackageKind::Profile => {
            cfg.delete_profile(&item.id)?;
        }
        PackageKind::Style => {
            if let Some(ref p) = item.path {
                remove_file_if_exists(p)?;
            }
            if cfg.settings.style.active_style == item.id {
                cfg.settings.style.active_style = String::from("default.css");
                cfg.save_settings()?;
                cfg.sync_stylesheet();
            }
        }
        PackageKind::IconPack => {
            if let Some(ref p) = item.path {
                remove_dir_if_exists(p)?;
            }
            cfg.icon_packs.retain(|p| p.id != item.id);
        }
        PackageKind::Script => {
            if let Some(ref p) = item.path {
                remove_file_if_exists(p)?;
            }
        }
        PackageKind::Unknown => {
            if let Some(ref p) = item.path {
                remove_file_if_exists(p)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_patcher::{PackageMetadata, PatchLedger, PatchManifest};
    use genubar_platform::fs_utils::write_text_file;

    #[test]
    fn collects_and_deletes_user_installed_items_while_protecting_builtin() {
        let tmp_base = std::env::temp_dir().join("genubar_test_installed_catalog");
        let _ = fs::remove_dir_all(&tmp_base);
        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();

        let custom_css = tmp_base.join("styles/custom_dark.css");
        write_text_file(
            &custom_css,
            "/* ==GenubarStyle==\n * @name Custom Dark\n * @version 1.2.0\n * @author alice\n ==/GenubarStyle== */\n",
        )
        .unwrap();

        let custom_patch_file = tmp_base.join("patches/user_patch.toml");
        write_text_file(&custom_patch_file, "[meta]\nid = \"user_patch\"\n").unwrap();

        let script_file = tmp_base.join("scripts/custom_menu.py");
        write_text_file(
            &script_file,
            "# ==GenubarScript==\n# @id custom_menu\n# @name Custom Menu\n# @version 1.0.0\n# @author bob\n# ==/GenubarScript==\ndef get_menu(): return []\n",
        )
        .unwrap();

        let mut patcher = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: vec![
                PatchManifest {
                    meta: PackageMetadata {
                        id: String::from("builtin_patch"),
                        name: String::from("BuiltIn Patch"),
                        origin: PackageOrigin::BuiltIn,
                        ..Default::default()
                    },
                    detection: Default::default(),
                    steps: Vec::new(),
                },
                PatchManifest {
                    meta: PackageMetadata {
                        id: String::from("user_patch"),
                        name: String::from("User Patch"),
                        origin: PackageOrigin::ImportedLocal,
                        ..Default::default()
                    },
                    detection: Default::default(),
                    steps: Vec::new(),
                },
            ],
            ledger: PatchLedger::default(),
        };

        let items = collect_all_installed_items(&cfg, &patcher);

        let builtin_item = items
            .iter()
            .find(|i| i.kind == PackageKind::Patch && i.id == "builtin_patch")
            .unwrap()
            .clone();
        assert!(!builtin_item.can_delete);
        assert!(delete_installed_item(&mut cfg, &mut patcher, &builtin_item).is_err());

        let user_patch = items
            .iter()
            .find(|i| i.kind == PackageKind::Patch && i.id == "user_patch")
            .unwrap()
            .clone();
        assert!(user_patch.can_delete);
        delete_installed_item(&mut cfg, &mut patcher, &user_patch).unwrap();
        assert!(!custom_patch_file.exists());
        assert!(!patcher.patches.iter().any(|p| p.meta.id == "user_patch"));

        let user_style = items
            .iter()
            .find(|i| i.kind == PackageKind::Style && i.id == "custom_dark.css")
            .unwrap()
            .clone();
        delete_installed_item(&mut cfg, &mut patcher, &user_style).unwrap();
        assert!(!custom_css.exists());

        let user_script = items
            .iter()
            .find(|i| i.kind == PackageKind::Script && i.id == "custom_menu")
            .unwrap()
            .clone();
        delete_installed_item(&mut cfg, &mut patcher, &user_script).unwrap();
        assert!(!script_file.exists());

        let _ = fs::remove_dir_all(&tmp_base);
    }
}
