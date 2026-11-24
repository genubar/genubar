use crate::inherit::InheritanceEngine;
use crate::postprocess::sanitize_separators;
use crate::static_builder::StaticMenuBuilder;
use genubar_config::MenuNodeDef;
use genubar_core::{MenuItemType, MenuNode, MenuSourceTier, clean_label};
use std::collections::HashMap;

pub struct MandatoryMerger;

impl MandatoryMerger {
    pub fn alias_exists_in_live(
        aliases: &[Vec<String>],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
    ) -> bool {
        aliases.iter().any(|alias_path| {
            let norm_key: Vec<String> = alias_path
                .iter()
                .map(|seg| clean_label(seg).to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            !norm_key.is_empty() && live_root_index.contains_key(&norm_key)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn merge_column_into_output(
        out_columns: &mut Vec<MenuNode>,
        live_columns: &[MenuNode],
        label: &str,
        icon: Option<String>,
        enabled: bool,
        col_aliases: &[Vec<String>],
        items: &[MenuNodeDef],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) {
        let expanded_col_label = StaticMenuBuilder::expand_label(label, app_name);
        let candidate_col_names = Self::collect_candidate_names(&expanded_col_label, col_aliases);
        let filtered_defs =
            Self::filter_unclaimed_action_defs(items, &expanded_col_label, live_root_index);

        if let Some(existing_col) = out_columns
            .iter_mut()
            .find(|c| candidate_col_names.contains(&c.clean_label().to_lowercase()))
        {
            // Keep empty lazy submenus on AT-SPI2 / Canonical DBusMenu untouched so the frontend triggers
            // `FetchSubmenu`; `apply_app_menu_to_fetched_submenu` merges mandatory items once expanded.
            let is_lazy_unexpanded = existing_col.item_type == MenuItemType::Submenu
                && existing_col.children.is_empty()
                && existing_col.id > 0
                && matches!(tier, MenuSourceTier::Atspi | MenuSourceTier::CanonicalDbus);

            if is_lazy_unexpanded {
                return;
            }

            Self::merge_into_existing_column(
                existing_col,
                &filtered_defs,
                live_root_index,
                app_name,
                tier,
                next_id,
            );
            sanitize_separators(existing_col);
            return;
        }

        let live_col_opt = live_columns
            .iter()
            .find(|c| candidate_col_names.contains(&c.clean_label().to_lowercase()));

        if let Some(live_col) = live_col_opt
            && live_col.item_type == MenuItemType::Submenu
            && live_col.children.is_empty()
            && live_col.id > 0
            && matches!(tier, MenuSourceTier::Atspi | MenuSourceTier::CanonicalDbus)
        {
            out_columns.push(live_col.clone());
            return;
        }

        let live_col_children = live_col_opt.map(|c| c.children.as_slice()).unwrap_or(&[]);

        let merged_children = InheritanceEngine::evaluate_defs_against_live(
            &filtered_defs,
            live_col_children,
            live_root_index,
            true,
            app_name,
            tier,
            next_id,
        );

        let col_id = *next_id;
        *next_id -= 1;
        let mut new_col = MenuNode {
            id: col_id,
            label: expanded_col_label,
            enabled,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon,
            origin_path: None,
            children: merged_children,
        };
        sanitize_separators(&mut new_col);

        if !new_col.children.is_empty() {
            out_columns.push(new_col);
        }
    }

    pub(crate) fn collect_candidate_names(
        expanded_label: &str,
        col_aliases: &[Vec<String>],
    ) -> Vec<String> {
        let mut names = vec![clean_label(expanded_label).to_lowercase()];
        for alias in col_aliases {
            if let Some(first) = alias.first() {
                names.push(clean_label(first).to_lowercase());
            }
        }
        names
    }

    pub(crate) fn filter_unclaimed_action_defs(
        items: &[MenuNodeDef],
        expanded_col_label: &str,
        live_root_index: &HashMap<Vec<String>, MenuNode>,
    ) -> Vec<MenuNodeDef> {
        items
            .iter()
            .filter(|def| match def {
                MenuNodeDef::Action {
                    label: act_lbl,
                    match_aliases,
                    ..
                } => {
                    let mut all_aliases = match_aliases.clone();
                    all_aliases.push(vec![expanded_col_label.to_string(), act_lbl.clone()]);
                    !Self::alias_exists_in_live(&all_aliases, live_root_index)
                }
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn merge_into_existing_column(
        existing_col: &mut MenuNode,
        filtered_defs: &[MenuNodeDef],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
        tier: MenuSourceTier,
        next_id: &mut i32,
    ) {
        let has_inherit_others = filtered_defs
            .iter()
            .any(|d| matches!(d, MenuNodeDef::InheritOthers));

        if has_inherit_others {
            existing_col.children = InheritanceEngine::evaluate_defs_against_live(
                filtered_defs,
                &existing_col.children,
                live_root_index,
                false,
                app_name,
                tier,
                next_id,
            );
        } else {
            let built =
                StaticMenuBuilder::build_static_list(filtered_defs, app_name, tier, next_id);
            if !built.is_empty() {
                if !existing_col.children.is_empty() {
                    let sep_id = *next_id;
                    *next_id -= 1;
                    existing_col.children.push(MenuNode {
                        id: sep_id,
                        label: String::new(),
                        enabled: true,
                        visible: true,
                        item_type: MenuItemType::Separator,
                        shortcut: None,
                        action: None,
                        icon: None,
                        origin_path: None,
                        children: vec![],
                    });
                }
                existing_col.children.extend(built);
            }
        }
    }
}
