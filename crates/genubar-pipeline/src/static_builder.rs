use genubar_config::MenuNodeDef;
use genubar_core::{MenuItemType, MenuNode, MenuSourceTier};

pub struct StaticMenuBuilder;

impl StaticMenuBuilder {
    pub fn expand_label(raw: &str, app_name: Option<&str>) -> String {
        let with_os = if raw.contains("{os_name}") {
            raw.replace("{os_name}", genubar_platform::detect_os_name())
        } else {
            raw.to_string()
        };
        if let Some(name) = app_name {
            with_os.replace("{app_name}", name)
        } else {
            with_os
        }
    }

    pub fn tier_matches(
        only_tiers: &Option<Vec<MenuSourceTier>>,
        current_tier: MenuSourceTier,
    ) -> bool {
        match only_tiers {
            Some(list) => list.contains(&current_tier),
            None => true,
        }
    }

    pub fn build_static_tree(
        defs: &[MenuNodeDef],
        app_name: Option<&str>,
        tier: MenuSourceTier,
        start_id: i32,
    ) -> MenuNode {
        let mut next_id = start_id;
        let children = Self::build_static_list(defs, app_name, tier, &mut next_id);
        MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children,
        }
    }

    pub fn build_static_list(
        defs: &[MenuNodeDef],
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) -> Vec<MenuNode> {
        let mut out = Vec::new();
        for def in defs {
            let id = *next_id;
            *next_id -= 1;
            if let Some(node) = Self::build_single_static_node(def, id, app_name, tier, next_id) {
                out.push(node);
            }
        }
        out
    }

    fn build_single_static_node(
        def: &MenuNodeDef,
        id: i32,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) -> Option<MenuNode> {
        match def {
            MenuNodeDef::InheritOthers | MenuNodeDef::Unsupported => None,
            MenuNodeDef::Inherit {
                from,
                match_labels,
                origin_path,
                label,
                icon,
                enabled,
                visible,
                shortcut,
                items,
                ..
            } => {
                if !*visible {
                    return None;
                }
                let fallback_lbl = if !from.is_empty() {
                    from.as_str()
                } else {
                    match_labels.first().map(String::as_str).unwrap_or("Item")
                };
                let final_label =
                    Self::expand_label(label.as_deref().unwrap_or(fallback_lbl), app_name);
                let children = items
                    .as_ref()
                    .map(|sub| Self::build_static_list(sub, app_name, tier, next_id))
                    .unwrap_or_default();
                let item_type = if !children.is_empty() {
                    MenuItemType::Submenu
                } else {
                    MenuItemType::Standard
                };
                Some(MenuNode {
                    id,
                    label: final_label,
                    enabled: *enabled,
                    visible: *visible,
                    item_type,
                    shortcut: shortcut.clone(),
                    action: None,
                    icon: icon.clone(),
                    origin_path: origin_path.clone(),
                    children,
                })
            }
            MenuNodeDef::Separator {
                visible,
                only_tiers,
            } => {
                if !*visible || !Self::tier_matches(only_tiers, tier) {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: String::new(),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::Separator,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: vec![],
                })
            }
            MenuNodeDef::Action {
                label,
                action,
                icon,
                enabled,
                visible,
                shortcut,
                only_tiers,
                ..
            } => {
                if !*visible || !Self::tier_matches(only_tiers, tier) {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: Self::expand_label(label, app_name),
                    enabled: *enabled,
                    visible: true,
                    item_type: MenuItemType::Standard,
                    shortcut: shortcut.clone(),
                    action: action.clone(),
                    icon: icon.clone(),
                    origin_path: None,
                    children: vec![],
                })
            }
            MenuNodeDef::Check {
                label,
                checked,
                action,
                icon,
                enabled,
                visible,
                shortcut,
            } => {
                if !*visible {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: Self::expand_label(label, app_name),
                    enabled: *enabled,
                    visible: true,
                    item_type: MenuItemType::Check(*checked),
                    shortcut: shortcut.clone(),
                    action: action.clone(),
                    icon: icon.clone(),
                    origin_path: None,
                    children: vec![],
                })
            }
            MenuNodeDef::Radio {
                label,
                selected,
                action,
                icon,
                enabled,
                visible,
                shortcut,
            } => {
                if !*visible {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: Self::expand_label(label, app_name),
                    enabled: *enabled,
                    visible: true,
                    item_type: MenuItemType::Radio(*selected),
                    shortcut: shortcut.clone(),
                    action: action.clone(),
                    icon: icon.clone(),
                    origin_path: None,
                    children: vec![],
                })
            }
            MenuNodeDef::Submenu {
                label,
                icon,
                enabled,
                visible,
                only_tiers,
                items,
                ..
            } => {
                if !*visible || !Self::tier_matches(only_tiers, tier) {
                    return None;
                }
                let children = Self::build_static_list(items, app_name, tier, next_id);
                Some(MenuNode {
                    id,
                    label: Self::expand_label(label, app_name),
                    enabled: *enabled,
                    visible: true,
                    item_type: MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    icon: icon.clone(),
                    origin_path: None,
                    children,
                })
            }
            MenuNodeDef::SearchBar {
                placeholder,
                visible,
                only_tiers,
            } => {
                if !*visible || !Self::tier_matches(only_tiers, tier) {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: Self::expand_label(placeholder, app_name),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::SearchBar,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: vec![],
                })
            }
            MenuNodeDef::WindowList {
                min_windows_to_show,
                max_title_length,
                visible,
                only_tiers,
            } => {
                if !*visible || !Self::tier_matches(only_tiers, tier) {
                    return None;
                }
                Some(MenuNode {
                    id,
                    label: String::from("Open Windows"),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::WindowList {
                        min_windows_to_show: *min_windows_to_show,
                        max_title_length: *max_title_length,
                    },
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: vec![],
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_config::DesktopMenuConfig;

    #[test]
    fn expands_os_name_and_resolves_desktop_system_title() {
        let detected = genubar_platform::detect_os_name();
        assert_eq!(StaticMenuBuilder::expand_label("{os_name}", None), detected);
        assert_eq!(
            StaticMenuBuilder::expand_label("{app_name} on {os_name}", Some("Files")),
            format!("Files on {detected}")
        );

        let default_cfg = DesktopMenuConfig::default();
        assert_eq!(default_cfg.resolved_system_title(), detected);

        let custom_cfg = DesktopMenuConfig {
            system_title: Some(String::from("  My Distro  ")),
            ..Default::default()
        };
        assert_eq!(custom_cfg.resolved_system_title(), "My Distro");

        let defs = vec![
            MenuNodeDef::Unsupported,
            MenuNodeDef::Submenu {
                label: String::from("View"),
                icon: None,
                enabled: true,
                visible: true,
                only_tiers: None,
                merge_with_live: false,
                match_aliases: vec![],
                items: vec![
                    MenuNodeDef::Unsupported,
                    MenuNodeDef::Action {
                        label: String::from("Refresh"),
                        action: Some(String::from("win.refresh")),
                        icon: None,
                        enabled: true,
                        visible: true,
                        shortcut: None,
                        only_tiers: None,
                        match_aliases: vec![],
                    },
                ],
            },
        ];
        let built = StaticMenuBuilder::build_static_tree(
            &defs,
            Some("Future"),
            MenuSourceTier::AppProfile,
            1,
        );
        assert_eq!(built.children.len(), 1);
        assert_eq!(built.children[0].children.len(), 1);
        assert_eq!(built.children[0].children[0].label, "Refresh");
    }
}
