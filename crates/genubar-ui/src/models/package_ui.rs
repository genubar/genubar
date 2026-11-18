use genubar_packages::{PackageKind, RegistrySourceStatus, RemotePackageEntry};

/// UI-specific presentation metadata for each [`PackageKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackageKindUiMeta {
    badge: &'static str,
    tab_id: &'static str,
    tab_title: &'static str,
}

const fn package_kind_meta(kind: PackageKind) -> PackageKindUiMeta {
    match kind {
        PackageKind::Bundle => PackageKindUiMeta {
            badge: "Bundle",
            tab_id: "bundles",
            tab_title: "Bundles",
        },
        PackageKind::Patch => PackageKindUiMeta {
            badge: "Patch",
            tab_id: "patches",
            tab_title: "Patches",
        },
        PackageKind::Profile => PackageKindUiMeta {
            badge: "Profile",
            tab_id: "profiles",
            tab_title: "Profiles",
        },
        PackageKind::Style => PackageKindUiMeta {
            badge: "Style",
            tab_id: "styles",
            tab_title: "Styles",
        },
        PackageKind::IconPack => PackageKindUiMeta {
            badge: "Icon Pack",
            tab_id: "icons",
            tab_title: "Icons",
        },
        PackageKind::Script => PackageKindUiMeta {
            badge: "Python Script",
            tab_id: "system_menus",
            tab_title: "System Menus",
        },
        PackageKind::Unknown => PackageKindUiMeta {
            badge: "Unknown",
            tab_id: "installed",
            tab_title: "Installed",
        },
    }
}

/// Human-readable short badge for a package kind in catalog and installed lists.
pub fn package_kind_badge(kind: PackageKind) -> &'static str {
    package_kind_meta(kind).badge
}

/// Stack child identifier for navigating to a package kind's management tab.
pub fn package_kind_tab_id(kind: PackageKind) -> &'static str {
    package_kind_meta(kind).tab_id
}

/// Display title of the destination tab for a package kind.
pub fn package_kind_tab_title(kind: PackageKind) -> &'static str {
    package_kind_meta(kind).tab_title
}

/// Maps the Marketplace category dropdown index to an optional [`PackageKind`] filter.
pub fn category_filter_from_index(idx: u32) -> Option<PackageKind> {
    match idx {
        1 => Some(PackageKind::Bundle),
        2 => Some(PackageKind::Patch),
        3 => Some(PackageKind::Profile),
        4 => Some(PackageKind::Style),
        5 => Some(PackageKind::IconPack),
        _ => None,
    }
}

/// Maps the Installed Packages view dropdown index to an optional [`PackageKind`] filter.
pub fn installed_filter_from_index(idx: u32) -> Option<PackageKind> {
    match idx {
        1 => Some(PackageKind::Bundle),
        2 => Some(PackageKind::Patch),
        3 => Some(PackageKind::Profile),
        4 => Some(PackageKind::Style),
        5 => Some(PackageKind::IconPack),
        6 => Some(PackageKind::Script),
        _ => None,
    }
}

/// Cached UI state for the remote catalog view (loaded entries, status text, current page).
#[derive(Debug, Clone)]
pub struct RegistryUiCache {
    pub entries: Vec<RemotePackageEntry>,
    pub statuses: Vec<RegistrySourceStatus>,
    pub summary: String,
    pub page: usize,
}

impl Default for RegistryUiCache {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            statuses: Vec::new(),
            summary: String::from(
                "Click 'Sync Catalog' in the top bar to load packages from registry sources",
            ),
            page: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_kind_ui_metadata_matches_expected_labels_and_tabs() {
        assert_eq!(package_kind_badge(PackageKind::Bundle), "Bundle");
        assert_eq!(package_kind_tab_id(PackageKind::Bundle), "bundles");
        assert_eq!(package_kind_tab_title(PackageKind::Bundle), "Bundles");

        assert_eq!(package_kind_badge(PackageKind::Patch), "Patch");
        assert_eq!(package_kind_tab_id(PackageKind::Patch), "patches");
        assert_eq!(package_kind_tab_title(PackageKind::Patch), "Patches");

        assert_eq!(package_kind_badge(PackageKind::Profile), "Profile");
        assert_eq!(package_kind_tab_id(PackageKind::Profile), "profiles");
        assert_eq!(package_kind_tab_title(PackageKind::Profile), "Profiles");

        assert_eq!(package_kind_badge(PackageKind::Style), "Style");
        assert_eq!(package_kind_tab_id(PackageKind::Style), "styles");
        assert_eq!(package_kind_tab_title(PackageKind::Style), "Styles");

        assert_eq!(package_kind_badge(PackageKind::IconPack), "Icon Pack");
        assert_eq!(package_kind_tab_id(PackageKind::IconPack), "icons");
        assert_eq!(package_kind_tab_title(PackageKind::IconPack), "Icons");

        assert_eq!(package_kind_badge(PackageKind::Script), "Python Script");
        assert_eq!(package_kind_tab_id(PackageKind::Script), "system_menus");
        assert_eq!(package_kind_tab_title(PackageKind::Script), "System Menus");

        assert_eq!(package_kind_badge(PackageKind::Unknown), "Unknown");
        assert_eq!(package_kind_tab_id(PackageKind::Unknown), "installed");
        assert_eq!(package_kind_tab_title(PackageKind::Unknown), "Installed");
    }

    #[test]
    fn dropdown_filter_indices_map_cleanly_to_package_kinds() {
        assert_eq!(category_filter_from_index(0), None);
        assert_eq!(category_filter_from_index(1), Some(PackageKind::Bundle));
        assert_eq!(category_filter_from_index(2), Some(PackageKind::Patch));
        assert_eq!(category_filter_from_index(3), Some(PackageKind::Profile));
        assert_eq!(category_filter_from_index(4), Some(PackageKind::Style));
        assert_eq!(category_filter_from_index(5), Some(PackageKind::IconPack));
        assert_eq!(category_filter_from_index(6), None);

        assert_eq!(installed_filter_from_index(0), None);
        assert_eq!(installed_filter_from_index(6), Some(PackageKind::Script));
        assert_eq!(installed_filter_from_index(99), None);
    }
}
