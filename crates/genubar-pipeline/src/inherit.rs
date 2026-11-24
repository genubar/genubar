use crate::mandatory::MandatoryMerger;
use crate::static_builder::StaticMenuBuilder;
use genubar_config::MenuNodeDef;
use genubar_core::{MenuItemType, MenuNode, MenuSourceTier, clean_label, wildcard_match};
use std::collections::{HashMap, HashSet};

pub struct InheritanceEngine;

impl InheritanceEngine {
    pub fn index_live_nodes(
        nodes: &[MenuNode],
        curr_path: &mut Vec<String>,
        map: &mut HashMap<Vec<String>, MenuNode>,
    ) {
        for n in nodes {
            let clean = n.clean_label().to_lowercase();
            if !clean.is_empty() {
                curr_path.push(clean);
                map.insert(curr_path.clone(), n.clone());
                Self::index_live_nodes(&n.children, curr_path, map);
                curr_path.pop();
            }
        }
    }

    pub fn evaluate_defs_against_live(
        defs: &[MenuNodeDef],
        live_nodes: &[MenuNode],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        sync_new_items: bool,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) -> Vec<MenuNode> {
        let mut out = Vec::new();
        let mut claimed_labels =
            Self::collect_claimed_labels(defs, live_nodes, live_root_index, app_name);
        let mut has_explicit_inherit_others = false;

        for def in defs {
            if matches!(def, MenuNodeDef::InheritOthers) {
                has_explicit_inherit_others = true;
                Self::append_unclaimed_live_nodes(&mut out, live_nodes, &mut claimed_labels);
            } else {
                Self::evaluate_single_def(
                    def,
                    &mut out,
                    live_nodes,
                    live_root_index,
                    app_name,
                    tier,
                    next_id,
                );
            }
        }

        if !has_explicit_inherit_others && sync_new_items {
            Self::append_unclaimed_live_nodes(&mut out, live_nodes, &mut claimed_labels);
        }

        out
    }

    fn collect_claimed_labels(
        defs: &[MenuNodeDef],
        live_nodes: &[MenuNode],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
    ) -> HashSet<String> {
        let mut claimed = HashSet::new();
        for def in defs {
            match def {
                MenuNodeDef::Inherit {
                    from,
                    match_labels,
                    match_aliases,
                    origin_path,
                    ..
                } => {
                    for live in live_nodes {
                        let live_clean = live.clean_label();
                        let hit = (!from.is_empty() && wildcard_match(from, &live_clean))
                            || match_labels
                                .iter()
                                .any(|pat| wildcard_match(pat, &live_clean));
                        if hit {
                            claimed.insert(live_clean.to_lowercase());
                        }
                    }
                    if let Some(matched) = Self::find_inherited_live_node(
                        from,
                        match_labels,
                        match_aliases,
                        origin_path,
                        live_nodes,
                        live_root_index,
                    ) {
                        claimed.insert(matched.clean_label().to_lowercase());
                    }
                }
                MenuNodeDef::Submenu {
                    label,
                    merge_with_live: true,
                    match_aliases,
                    ..
                } => {
                    let expanded = StaticMenuBuilder::expand_label(label, app_name);
                    claimed.insert(clean_label(&expanded).to_lowercase());
                    for alias in match_aliases {
                        if let Some(first) = alias.first() {
                            claimed.insert(clean_label(first).to_lowercase());
                        }
                    }
                }
                _ => {}
            }
        }
        claimed
    }

    fn append_unclaimed_live_nodes(
        out: &mut Vec<MenuNode>,
        live_nodes: &[MenuNode],
        claimed_labels: &mut HashSet<String>,
    ) {
        for live in live_nodes {
            let clean = live.clean_label().to_lowercase();
            let keep = (clean.is_empty() && live.item_type == MenuItemType::Separator)
                || (!clean.is_empty() && claimed_labels.insert(clean));
            if keep {
                out.push(live.clone());
            }
        }
    }

    fn evaluate_single_def(
        def: &MenuNodeDef,
        out: &mut Vec<MenuNode>,
        live_nodes: &[MenuNode],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) {
        match def {
            MenuNodeDef::InheritOthers | MenuNodeDef::Unsupported => {}
            MenuNodeDef::Inherit { visible: false, .. } => {}
            MenuNodeDef::Inherit {
                from,
                match_labels,
                match_aliases,
                origin_path,
                label,
                icon,
                enabled,
                shortcut,
                sync_new_items,
                items,
                ..
            } => {
                if let Some(live) = Self::find_inherited_live_node(
                    from,
                    match_labels,
                    match_aliases,
                    origin_path,
                    live_nodes,
                    live_root_index,
                ) {
                    out.push(Self::build_inherited_node(
                        live,
                        label.as_deref(),
                        icon.clone(),
                        *enabled,
                        shortcut.clone(),
                        *sync_new_items,
                        items,
                        live_root_index,
                        app_name,
                        tier,
                        next_id,
                    ));
                }
            }
            MenuNodeDef::Submenu {
                label,
                icon,
                enabled,
                visible,
                only_tiers,
                merge_with_live: true,
                match_aliases,
                items,
            } => {
                if *visible && StaticMenuBuilder::tier_matches(only_tiers, tier) {
                    MandatoryMerger::merge_column_into_output(
                        out,
                        live_nodes,
                        label,
                        icon.clone(),
                        *enabled,
                        match_aliases,
                        items,
                        live_root_index,
                        app_name,
                        tier,
                        next_id,
                    );
                }
            }
            MenuNodeDef::Action { match_aliases, .. }
                if !match_aliases.is_empty()
                    && MandatoryMerger::alias_exists_in_live(match_aliases, live_root_index) => {}
            other_static => {
                let built = StaticMenuBuilder::build_static_list(
                    std::slice::from_ref(other_static),
                    app_name,
                    tier,
                    next_id,
                );
                out.extend(built);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_inherited_node(
        live: &MenuNode,
        override_label: Option<&str>,
        override_icon: Option<String>,
        override_enabled: bool,
        override_shortcut: Option<Vec<String>>,
        child_sync: bool,
        sub_defs_opt: &Option<Vec<MenuNodeDef>>,
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) -> MenuNode {
        let final_label =
            StaticMenuBuilder::expand_label(override_label.unwrap_or(&live.label), app_name);
        let final_enabled = if !override_enabled {
            false
        } else {
            live.enabled
        };
        let final_shortcut = override_shortcut.or_else(|| live.shortcut.clone());
        let final_icon = override_icon.or_else(|| live.icon.clone());

        let is_lazy_unexpanded = live.item_type == MenuItemType::Submenu
            && live.children.is_empty()
            && matches!(tier, MenuSourceTier::Atspi | MenuSourceTier::CanonicalDbus);

        let final_children = match sub_defs_opt {
            None => live.children.clone(),
            Some(_) if is_lazy_unexpanded => Vec::new(),
            Some(sub_defs) => Self::evaluate_defs_against_live(
                sub_defs,
                &live.children,
                live_root_index,
                child_sync,
                app_name,
                tier,
                next_id,
            ),
        };

        MenuNode {
            id: live.id,
            label: final_label,
            enabled: final_enabled,
            visible: true,
            item_type: live.item_type.clone(),
            shortcut: final_shortcut,
            action: live.action.clone(),
            icon: final_icon,
            origin_path: live.origin_path.clone(),
            children: final_children,
        }
    }

    fn find_inherited_live_node<'a>(
        from: &str,
        match_labels: &[String],
        match_aliases: &[Vec<String>],
        origin_path: &Option<Vec<String>>,
        live_nodes: &'a [MenuNode],
        live_root_index: &'a HashMap<Vec<String>, MenuNode>,
    ) -> Option<&'a MenuNode> {
        let mut candidates = Vec::new();
        if !from.is_empty() {
            candidates.push(clean_label(from).to_lowercase());
        }
        for lbl in match_labels {
            let norm = clean_label(lbl).to_lowercase();
            if !norm.is_empty() && !candidates.contains(&norm) {
                candidates.push(norm);
            }
        }

        for pat in &candidates {
            if let Some(found) = live_nodes
                .iter()
                .find(|n| wildcard_match(pat, &n.clean_label()))
            {
                return Some(found);
            }
        }

        if let Some(orig) = origin_path {
            let key: Vec<String> = orig.iter().map(|s| clean_label(s).to_lowercase()).collect();
            if let Some(found) = live_root_index.get(&key) {
                return Some(found);
            }
        }

        for alias_path in match_aliases {
            let key: Vec<String> = alias_path
                .iter()
                .map(|s| clean_label(s).to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if let Some(found) = live_root_index.get(&key) {
                return Some(found);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        id: i32,
        label: &str,
        item_type: MenuItemType,
        action: Option<&str>,
        children: Vec<MenuNode>,
    ) -> MenuNode {
        MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type,
            shortcut: None,
            action: action.map(str::to_string),
            icon: None,
            origin_path: None,
            children,
        }
    }

    #[test]
    fn evaluates_aliases_overrides_visibility_merge_and_inherit_others() {
        let live_nodes = vec![
            node(
                10,
                "_View",
                MenuItemType::Submenu,
                None,
                vec![
                    node(
                        11,
                        "_Zoom In",
                        MenuItemType::Standard,
                        Some("win.zoom-in"),
                        vec![],
                    ),
                    node(
                        12,
                        "_Hidden Secret",
                        MenuItemType::Standard,
                        Some("win.secret"),
                        vec![],
                    ),
                ],
            ),
            node(20, "_Help", MenuItemType::Submenu, None, vec![]),
        ];

        let mut live_index = HashMap::new();
        let mut path = Vec::new();
        InheritanceEngine::index_live_nodes(&live_nodes, &mut path, &mut live_index);

        let defs = vec![
            MenuNodeDef::Submenu {
                label: String::from("{app_name}"),
                icon: None,
                enabled: true,
                visible: true,
                only_tiers: None,
                merge_with_live: true,
                match_aliases: vec![],
                items: vec![
                    MenuNodeDef::Inherit {
                        from: String::new(),
                        match_labels: vec![],
                        match_aliases: vec![vec![String::from("View"), String::from("Zoom In")]],
                        origin_path: None,
                        label: Some(String::from("Larger Text")),
                        icon: Some(String::from("zoom-in-symbolic")),
                        enabled: true,
                        visible: true,
                        shortcut: Some(vec![String::from("Ctrl+Plus")]),
                        sync_new_items: false,
                        items: None,
                    },
                    MenuNodeDef::Action {
                        label: String::from("Quit"),
                        action: Some(String::from("sys:quit_app")),
                        shortcut: None,
                        icon: None,
                        enabled: true,
                        visible: true,
                        only_tiers: None,
                        match_aliases: vec![],
                    },
                ],
            },
            MenuNodeDef::Inherit {
                from: String::from("View"),
                match_labels: vec![],
                match_aliases: vec![],
                origin_path: None,
                label: None,
                icon: None,
                enabled: true,
                visible: true,
                shortcut: None,
                sync_new_items: true,
                items: Some(vec![
                    MenuNodeDef::Inherit {
                        from: String::from("Hidden Secret"),
                        match_labels: vec![],
                        match_aliases: vec![],
                        origin_path: None,
                        label: None,
                        icon: None,
                        enabled: true,
                        visible: false,
                        shortcut: None,
                        sync_new_items: false,
                        items: None,
                    },
                    MenuNodeDef::InheritOthers,
                ]),
            },
            MenuNodeDef::InheritOthers,
        ];

        let mut next_id = 9000;
        let out = InheritanceEngine::evaluate_defs_against_live(
            &defs,
            &live_nodes,
            &live_index,
            true,
            Some("Notes"),
            MenuSourceTier::GtkNative,
            &mut next_id,
        );

        assert_eq!(out.len(), 3);
        assert_eq!(out[0].label, "Notes");
        assert_eq!(out[0].children[0].label, "Larger Text");
        assert_eq!(out[0].children[0].icon.as_deref(), Some("zoom-in-symbolic"));
        assert_eq!(
            out[0].children[0].shortcut,
            Some(vec![String::from("Ctrl+Plus")])
        );

        // Inside inherited View column: Hidden Secret is suppressed while Zoom In remains via InheritOthers.
        assert_eq!(out[1].clean_label(), "View");
        let view_labels: Vec<String> = out[1].children.iter().map(MenuNode::clean_label).collect();
        assert_eq!(view_labels, vec!["Zoom In"]);

        // Help column comes last via top-level InheritOthers without duplicating View.
        assert_eq!(out[2].clean_label(), "Help");
    }
}
