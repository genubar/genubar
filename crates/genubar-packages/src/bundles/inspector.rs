use super::{BundleManager, BundleOrchestrator};
use genubar_config::{BundleManifest, BundleStatusView, ConfigManager, StyleMetadata};
use genubar_core::PackageKind;
use genubar_patcher::PatchManager;

#[derive(Debug, Clone)]
pub struct BundleComponentItem {
    pub kind: PackageKind,
    pub id: String,
    pub name: String,
    pub file_or_ref: String,
    pub installed: bool,
    pub active: bool,
    pub status_label: String,
}

#[derive(Debug, Clone)]
pub struct BundleDetailedView {
    pub status: BundleStatusView,
    pub components: Vec<BundleComponentItem>,
    pub active_components: usize,
    pub applied_patches: usize,
    pub total_patches: usize,
}

pub fn inspect_bundles(cfg: &ConfigManager, patcher: &PatchManager) -> Vec<BundleDetailedView> {
    let styles = cfg.styles.list_styles();
    let bundle_mgr = BundleManager::new(cfg.base_dir().to_path_buf());
    let mut views = Vec::new();

    for status in bundle_mgr.list_bundles() {
        let m = &status.manifest;
        let mut components = Vec::new();

        let (applied_patches, total_patches) =
            inspect_patches(cfg, patcher, m, &mut components);
        inspect_profiles(cfg, m, &mut components);
        inspect_styles(cfg, m, &styles, &mut components);
        inspect_icon_packs(cfg, m, &mut components);

        let active_components = components.iter().filter(|c| c.active).count();
        views.push(BundleDetailedView {
            status,
            components,
            active_components,
            applied_patches,
            total_patches,
        });
    }

    views
}

fn inspect_patches(
    cfg: &ConfigManager,
    patcher: &PatchManager,
    m: &BundleManifest,
    out: &mut Vec<BundleComponentItem>,
) -> (usize, usize) {
    let matched_patches = BundleOrchestrator::matching_patches(patcher, &m.id, &m.patches);
    let mut applied_patches = 0usize;
    let total_patches = matched_patches.len();

    if matched_patches.is_empty() {
        for pf in &m.patches {
            let installed = cfg.base_dir().join("patches").join(pf).is_file();
            out.push(BundleComponentItem {
                kind: PackageKind::Patch,
                id: pf.trim_end_matches(".toml").to_string(),
                name: pf.clone(),
                file_or_ref: format!("patches/{pf}"),
                installed,
                active: false,
                status_label: String::from(if installed {
                    "Saved (Not Applied)"
                } else {
                    "Missing"
                }),
            });
        }
        return (applied_patches, total_patches);
    }

    for pm in matched_patches {
        let is_applied = patcher.ledger.applied_patches.contains(&pm.meta.id);
        if is_applied {
            applied_patches += 1;
        }
        let status_label = if is_applied {
            String::from("Applied")
        } else if !pm.is_fully_compatible() {
            pm.incompatibility_reason()
                .unwrap_or_else(|| String::from("Requires Update"))
        } else {
            String::from("Not Applied")
        };
        out.push(BundleComponentItem {
            kind: PackageKind::Patch,
            id: pm.meta.id.clone(),
            name: pm.meta.name.clone(),
            file_or_ref: format!("id={}", pm.meta.id),
            installed: true,
            active: is_applied,
            status_label,
        });
    }

    (applied_patches, total_patches)
}

fn inspect_profiles(
    cfg: &ConfigManager,
    m: &BundleManifest,
    out: &mut Vec<BundleComponentItem>,
) {
    for prof_file in &m.profiles {
        let stem = prof_file.trim_end_matches(".toml");
        if let Some(p) = cfg.profiles.iter().find(|pr| {
            pr.id == stem
                || pr.file_path.file_name().and_then(|n| n.to_str()) == Some(prof_file.as_str())
                || format!("{}.toml", pr.id) == *prof_file
        }) {
            out.push(BundleComponentItem {
                kind: PackageKind::Profile,
                id: p.id.clone(),
                name: p.name.clone(),
                file_or_ref: format!("profiles/{prof_file}"),
                installed: true,
                active: p.enabled,
                status_label: String::from(if p.enabled { "Enabled" } else { "Disabled" }),
            });
        } else {
            let exists = cfg.base_dir().join("profiles").join(prof_file).is_file();
            out.push(BundleComponentItem {
                kind: PackageKind::Profile,
                id: stem.to_string(),
                name: stem.to_string(),
                file_or_ref: format!("profiles/{prof_file}"),
                installed: exists,
                active: false,
                status_label: String::from(if exists { "Installed" } else { "Missing" }),
            });
        }
    }
}

fn inspect_styles(
    cfg: &ConfigManager,
    m: &BundleManifest,
    styles: &[StyleMetadata],
    out: &mut Vec<BundleComponentItem>,
) {
    for st_file in &m.styles {
        let st_opt = styles
            .iter()
            .find(|s| s.file_name == *st_file || format!("{}.css", s.id) == *st_file);
        let installed = st_opt.is_some() || cfg.base_dir().join("styles").join(st_file).is_file();
        let active = cfg.settings.style.active_style == *st_file;
        let name = st_opt
            .map(|s| s.name.clone())
            .unwrap_or_else(|| st_file.clone());
        let status_label = if !installed {
            "Missing"
        } else if active {
            "Active Style"
        } else {
            "Installed (Inactive)"
        };
        out.push(BundleComponentItem {
            kind: PackageKind::Style,
            id: st_file.trim_end_matches(".css").to_string(),
            name,
            file_or_ref: format!("styles/{st_file}"),
            installed,
            active,
            status_label: String::from(status_label),
        });
    }
}

fn inspect_icon_packs(
    cfg: &ConfigManager,
    m: &BundleManifest,
    out: &mut Vec<BundleComponentItem>,
) {
    for pack_id in &m.icon_packs {
        if let Some(ic) = cfg.icon_packs.iter().find(|p| p.id == *pack_id) {
            out.push(BundleComponentItem {
                kind: PackageKind::IconPack,
                id: ic.id.clone(),
                name: ic.name.clone(),
                file_or_ref: format!("icons/{pack_id}/pack.toml"),
                installed: true,
                active: ic.enabled,
                status_label: String::from(if ic.enabled { "Enabled" } else { "Disabled" }),
            });
        } else {
            let exists = cfg
                .base_dir()
                .join("icons")
                .join(pack_id)
                .join("pack.toml")
                .is_file();
            out.push(BundleComponentItem {
                kind: PackageKind::IconPack,
                id: pack_id.clone(),
                name: pack_id.clone(),
                file_or_ref: format!("icons/{pack_id}/pack.toml"),
                installed: exists,
                active: false,
                status_label: String::from(if exists { "Installed" } else { "Missing" }),
            });
        }
    }
}
