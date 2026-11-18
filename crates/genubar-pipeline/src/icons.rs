use crate::icon_matcher::{IconEvalContext, IconMatchRuleExt};
use genubar_config::{IconPackManifest, IconsConfig};
use genubar_core::{ActiveWindowInfo, MenuItemType, MenuNode};

pub struct IconResolver;

impl IconResolver {
    pub fn apply_icons(
        root: &mut MenuNode,
        info: &ActiveWindowInfo,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
    ) {
        Self::apply_icons_with_options(root, info, icons_cfg, active_packs, true, true);
    }

    /// Applies icon rules while honoring global overrides for showing icons and auto-mapping standard icons.
    pub fn apply_icons_with_options(
        root: &mut MenuNode,
        info: &ActiveWindowInfo,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
        allow_icons: bool,
        allow_system_fallback: bool,
    ) {
        let mut current_path = Vec::new();
        for top_col in &mut root.children {
            let col_clean = top_col.clean_label();
            if !col_clean.is_empty() {
                current_path.push(col_clean);
                Self::populate_children_slice(
                    &mut top_col.children,
                    info,
                    &mut current_path,
                    icons_cfg,
                    active_packs,
                    allow_icons,
                    allow_system_fallback,
                );
                current_path.pop();
            }
        }
    }

    fn populate_children_slice(
        children: &mut [MenuNode],
        info: &ActiveWindowInfo,
        parent_path: &mut Vec<String>,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
        allow_icons: bool,
        allow_system_fallback: bool,
    ) {
        let sibling_labels: Vec<String> = children.iter().map(|c| c.clean_label()).collect();

        for (idx, node) in children.iter_mut().enumerate() {
            let clean_lbl = sibling_labels[idx].clone();
            let mut item_path = parent_path.clone();
            if !clean_lbl.is_empty() {
                item_path.push(clean_lbl.clone());
            }

            let resolved = Self::resolve_node_icon(
                node,
                &IconEvalContext {
                    info,
                    parent_path,
                    current_path: &item_path,
                    label: &clean_lbl,
                    action: node.action.as_deref(),
                    index_in_parent: idx,
                    sibling_labels: &sibling_labels,
                },
                icons_cfg,
                active_packs,
                allow_icons,
                allow_system_fallback,
            );
            node.icon = resolved;

            if !node.children.is_empty() {
                let pushed = if !clean_lbl.is_empty() {
                    parent_path.push(clean_lbl);
                    true
                } else {
                    false
                };
                Self::populate_children_slice(
                    &mut node.children,
                    info,
                    parent_path,
                    icons_cfg,
                    active_packs,
                    allow_icons,
                    allow_system_fallback,
                );
                if pushed {
                    parent_path.pop();
                }
            }
        }
    }

    fn resolve_node_icon(
        node: &MenuNode,
        ctx: &IconEvalContext<'_>,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
        allow_icons: bool,
        allow_system_fallback: bool,
    ) -> Option<String> {
        if !allow_icons
            || !icons_cfg.general.show_menu_icons
            || node.icon.as_deref() == Some("none")
        {
            return None;
        }

        if node.icon.is_some()
            || matches!(
                node.item_type,
                MenuItemType::Separator
                    | MenuItemType::SearchBar
                    | MenuItemType::WindowList { .. }
            )
        {
            return node.icon.clone();
        }

        for pack in active_packs.iter().filter(|p| p.enabled) {
            if let Some(rule) = pack.rules.iter().find(|r| r.matches(ctx)) {
                return rule.resolve_icon_value(Some(&pack.pack_dir));
            }
        }

        if allow_system_fallback && icons_cfg.general.use_system_fallback
            && let Some(rule) = icons_cfg.system_rules.iter().find(|r| r.matches(ctx)) {
                return rule.resolve_icon_value(None);
            }

        None
    }
}
