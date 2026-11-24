use crate::bundles::BundleManager;
use crate::models::{CatalogSearchItem, RemotePackageEntry};
use genubar_config::ConfigManager;
use genubar_core::PackageKind;
use genubar_patcher::PatchManager;

// Searches across all local & cached remote packages using unified `PackageKind` filtering.
pub fn search_packages(
    cfg: &ConfigManager,
    patcher: &PatchManager,
    remote_packages: &[RemotePackageEntry],
    query: &str,
    category: Option<PackageKind>,
) -> Vec<CatalogSearchItem> {
    let q = query.trim().to_lowercase();
    let mut results = Vec::new();
    let matches_kind = |k: PackageKind| category.is_none() || category == Some(k);

    if matches_kind(PackageKind::Bundle) {
        let bundle_mgr = BundleManager::new(cfg.base_dir().to_path_buf());
        for b in bundle_mgr.list_bundles() {
            if q.is_empty()
                || b.manifest.name.to_lowercase().contains(&q)
                || b.manifest.description.to_lowercase().contains(&q)
            {
                let badge = RemotePackageEntry::version_req_badge(
                    b.is_compatible,
                    b.manifest.min_app_version.as_deref(),
                );
                results.push(CatalogSearchItem {
                    kind: PackageKind::Bundle,
                    is_local: true,
                    title: format!("{} (v{})", b.manifest.name, b.manifest.version),
                    subtitle: format!("by {} • {}", b.manifest.author, b.manifest.description),
                    is_compatible: b.is_compatible,
                    incompatibility_badge: badge,
                });
            }
        }
    }

    if matches_kind(PackageKind::Patch) {
        for p in patcher.list_patch_cards() {
            let m = &p.manifest.meta;
            if q.is_empty()
                || m.name.to_lowercase().contains(&q)
                || m.description.to_lowercase().contains(&q)
                || m.tags.iter().any(|t| t.to_lowercase().contains(&q))
            {
                results.push(CatalogSearchItem {
                    kind: PackageKind::Patch,
                    is_local: true,
                    title: format!("{} (v{})", m.name, m.version),
                    subtitle: format!("by {} • {}", m.author, m.description),
                    is_compatible: p.is_compatible,
                    incompatibility_badge: p.incompatibility_reason,
                });
            }
        }
    }

    if matches_kind(PackageKind::Profile) {
        for pr in &cfg.profiles {
            if q.is_empty()
                || pr.name.to_lowercase().contains(&q)
                || pr.id.to_lowercase().contains(&q)
            {
                let compat = pr.is_compatible();
                let badge =
                    RemotePackageEntry::version_req_badge(compat, pr.min_app_version.as_deref());
                results.push(CatalogSearchItem {
                    kind: PackageKind::Profile,
                    is_local: true,
                    title: pr.name.clone(),
                    subtitle: format!("by {} • id={}", pr.author, pr.id),
                    is_compatible: compat,
                    incompatibility_badge: badge,
                });
            }
        }
    }

    if matches_kind(PackageKind::Style) {
        for st in cfg.styles.list_styles() {
            if q.is_empty()
                || st.name.to_lowercase().contains(&q)
                || st.file_name.to_lowercase().contains(&q)
            {
                let compat = st.is_compatible();
                let badge =
                    RemotePackageEntry::version_req_badge(compat, st.min_app_version.as_deref());
                results.push(CatalogSearchItem {
                    kind: PackageKind::Style,
                    is_local: true,
                    title: format!("{} ({})", st.name, st.file_name),
                    subtitle: format!("by {} • {}", st.author, st.description),
                    is_compatible: compat,
                    incompatibility_badge: badge,
                });
            }
        }
    }

    if matches_kind(PackageKind::IconPack) {
        for ic in &cfg.icon_packs {
            if q.is_empty()
                || ic.name.to_lowercase().contains(&q)
                || ic.id.to_lowercase().contains(&q)
            {
                let compat = ic.is_compatible();
                let badge =
                    RemotePackageEntry::version_req_badge(compat, ic.min_app_version.as_deref());
                results.push(CatalogSearchItem {
                    kind: PackageKind::IconPack,
                    is_local: true,
                    title: ic.name.clone(),
                    subtitle: format!("by {} • {} rules", ic.author, ic.rules.len()),
                    is_compatible: compat,
                    incompatibility_badge: badge,
                });
            }
        }
    }

    for rpkg in remote_packages {
        if !matches_kind(rpkg.kind) {
            continue;
        }
        if q.is_empty()
            || rpkg.name.to_lowercase().contains(&q)
            || rpkg.description.to_lowercase().contains(&q)
            || rpkg.tags.iter().any(|t| t.to_lowercase().contains(&q))
        {
            results.push(CatalogSearchItem {
                kind: rpkg.kind,
                is_local: false,
                title: format!("{} (v{}) [Registry]", rpkg.name, rpkg.version),
                subtitle: format!("by {} • {}", rpkg.author, rpkg.description),
                is_compatible: rpkg.is_compatible(),
                incompatibility_badge: rpkg.incompatibility_badge(),
            });
        }
    }

    results
}
