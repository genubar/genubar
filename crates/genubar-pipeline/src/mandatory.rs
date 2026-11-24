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
        Self::alias_exists_in_live_with_app(aliases, live_root_index, None)
    }

    pub fn alias_exists_in_live_with_app(
        aliases: &[Vec<String>],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
    ) -> bool {
        aliases.iter().any(|alias_path| {
            let raw_key: Vec<String> = alias_path
                .iter()
                .map(|seg| clean_label(seg).to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if !raw_key.is_empty() && live_root_index.contains_key(&raw_key) {
                return true;
            }
            if app_name.is_some() {
                let expanded_key: Vec<String> = alias_path
                    .iter()
                    .map(|seg| {
                        clean_label(&StaticMenuBuilder::expand_label(seg, app_name)).to_lowercase()
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                return !expanded_key.is_empty() && live_root_index.contains_key(&expanded_key);
            }
            false
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
        let candidate_col_names =
            Self::collect_candidate_names(&expanded_col_label, col_aliases, app_name);
        let filtered_defs = Self::filter_unclaimed_action_defs(
            items,
            &candidate_col_names,
            live_root_index,
            app_name,
        );

        if let Some(existing_col) = out_columns
            .iter_mut()
            .find(|c| candidate_col_names.contains(&c.clean_label().to_lowercase()))
        {
            if existing_col.label.contains("{app_name}") || existing_col.label.contains("{os_name}")
            {
                existing_col.label = expanded_col_label;
            }
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
        app_name: Option<&str>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        let mut push_unique = |raw: &str| {
            let norm = clean_label(raw).to_lowercase();
            if !norm.is_empty() && !names.contains(&norm) {
                names.push(norm);
            }
        };

        push_unique(expanded_label);
        for alias in col_aliases {
            if let Some(first) = alias.first() {
                push_unique(first);
                if app_name.is_some() {
                    let expanded = StaticMenuBuilder::expand_label(first, app_name);
                    push_unique(&expanded);
                }
            }
        }
        names
    }

    pub(crate) fn filter_unclaimed_action_defs(
        items: &[MenuNodeDef],
        candidate_col_names: &[String],
        live_root_index: &HashMap<Vec<String>, MenuNode>,
        app_name: Option<&str>,
    ) -> Vec<MenuNodeDef> {
        items
            .iter()
            .filter(|def| match def {
                MenuNodeDef::Action {
                    label: act_lbl,
                    match_aliases,
                    ..
                } => {
                    if Self::alias_exists_in_live_with_app(match_aliases, live_root_index, app_name)
                    {
                        return false;
                    }
                    let expanded_act_lbl = StaticMenuBuilder::expand_label(act_lbl, app_name);
                    let mut col_scoped_aliases = Vec::with_capacity(candidate_col_names.len() * 2);
                    for col_name in candidate_col_names {
                        col_scoped_aliases.push(vec![col_name.clone(), expanded_act_lbl.clone()]);
                        if expanded_act_lbl != *act_lbl {
                            col_scoped_aliases.push(vec![col_name.clone(), act_lbl.clone()]);
                        }
                    }
                    !Self::alias_exists_in_live(&col_scoped_aliases, live_root_index)
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

    /// Reconciles any literal `{app_name}` / `{os_name}` columns returned by a post-merge Python script:
    /// merges a literal `{app_name}` column into an existing expanded `app_name` column (above its
    /// existing system items), or expands placeholders in place when no separate app column exists.
    pub fn reconcile_app_name_columns(root: &mut MenuNode, app_name: &str) {
        let has_placeholder = root
            .children
            .iter()
            .any(|c| c.label.contains("{app_name}") || c.label.contains("{os_name}"));
        if !has_placeholder {
            return;
        }

        let norm_app = clean_label(app_name).to_lowercase();
        let existing_app_idx = root
            .children
            .iter()
            .position(|c| !norm_app.is_empty() && c.clean_label().to_lowercase() == norm_app);

        if let Some(target_idx) = existing_app_idx {
            let mut prepended: Vec<MenuNode> = Vec::new();
            let mut kept: Vec<MenuNode> = Vec::with_capacity(root.children.len());
            let mut adjusted_target_idx = target_idx;

            for (idx, mut col) in root.children.drain(..).enumerate() {
                let is_literal_app_placeholder =
                    col.label.trim().eq_ignore_ascii_case("{app_name}");
                if idx != target_idx && is_literal_app_placeholder {
                    if idx < target_idx {
                        adjusted_target_idx = adjusted_target_idx.saturating_sub(1);
                    }
                    prepended.extend(col.children);
                    continue;
                }
                if col.label.contains("{app_name}") || col.label.contains("{os_name}") {
                    col.label = StaticMenuBuilder::expand_label(&col.label, Some(app_name));
                }
                kept.push(col);
            }

            if !prepended.is_empty()
                && let Some(target_col) = kept.get_mut(adjusted_target_idx)
            {
                if !target_col.children.is_empty() {
                    prepended.push(MenuNode {
                        id: -940,
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
                prepended.append(&mut target_col.children);
                target_col.children = prepended;
                sanitize_separators(target_col);
            }
            root.children = kept;
        } else {
            for col in &mut root.children {
                if col.label.contains("{app_name}") || col.label.contains("{os_name}") {
                    col.label = StaticMenuBuilder::expand_label(&col.label, Some(app_name));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::MenuPipeline;
    use genubar_config::{AppMenuTemplateConfig, AppTemplateMode};
    use genubar_core::ActiveWindowInfo;
    use std::path::Path;

    fn make_node(
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

    fn app_menu_template() -> AppMenuTemplateConfig {
        AppMenuTemplateConfig {
            mode: AppTemplateMode::Static,
            sync_new_items: false,
            python_script: None,
            columns: vec![
                MenuNodeDef::Submenu {
                    label: String::from("{app_name}"),
                    icon: None,
                    enabled: true,
                    visible: true,
                    only_tiers: None,
                    merge_with_live: true,
                    match_aliases: vec![
                        vec![String::from("{app_name}")],
                        vec![String::from("App")],
                        vec![String::from("Application")],
                    ],
                    items: vec![
                        MenuNodeDef::InheritOthers,
                        MenuNodeDef::Separator {
                            visible: true,
                            only_tiers: None,
                        },
                        MenuNodeDef::Action {
                            label: String::from("Genubar Settings..."),
                            action: Some(String::from("cmd:genubar-settings")),
                            icon: None,
                            enabled: true,
                            visible: true,
                            shortcut: None,
                            only_tiers: None,
                            match_aliases: vec![],
                        },
                        MenuNodeDef::Separator {
                            visible: true,
                            only_tiers: None,
                        },
                        MenuNodeDef::Action {
                            label: String::from("Quit {app_name}"),
                            action: Some(String::from("sys:quit_app")),
                            icon: None,
                            enabled: true,
                            visible: true,
                            shortcut: Some(vec![String::from("Ctrl"), String::from("Q")]),
                            only_tiers: None,
                            match_aliases: vec![
                                vec![String::from("{app_name}"), String::from("Quit")],
                                vec![String::from("{app_name}"), String::from("Quit {app_name}")],
                                vec![String::from("App"), String::from("Quit")],
                                vec![String::from("Application"), String::from("Quit")],
                            ],
                        },
                    ],
                },
                MenuNodeDef::InheritOthers,
            ],
        }
    }

    #[test]
    fn merges_literal_app_name_and_alias_columns_above_system_actions() {
        let template = app_menu_template();
        let info = ActiveWindowInfo::default();

        // Case 1: Python script emits a literal `{app_name}` column
        let python_root = make_node(
            0,
            "Root",
            MenuItemType::Submenu,
            None,
            vec![
                make_node(
                    -900,
                    "{app_name}",
                    MenuItemType::Submenu,
                    None,
                    vec![make_node(
                        -901,
                        "Copy PID (42)",
                        MenuItemType::Standard,
                        Some("copy:42"),
                        vec![],
                    )],
                ),
                make_node(
                    1,
                    "File",
                    MenuItemType::Submenu,
                    None,
                    vec![make_node(
                        2,
                        "Quit",
                        MenuItemType::Standard,
                        Some("app.quit"),
                        vec![],
                    )],
                ),
            ],
        );

        let merged = MenuPipeline::apply_app_menu_template(
            python_root,
            &template,
            &info,
            MenuSourceTier::Gtk4Hook,
            "python3",
            Path::new("."),
            "Firefox",
        );

        let top_labels: Vec<&str> = merged.children.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(top_labels, vec!["Firefox", "File"]);

        let app_items: Vec<&str> = merged.children[0]
            .children
            .iter()
            .filter(|n| n.item_type != MenuItemType::Separator)
            .map(|n| n.label.as_str())
            .collect();
        // `File -> Quit` must not suppress `Quit Firefox` in the app column
        assert_eq!(
            app_items,
            vec!["Copy PID (42)", "Genubar Settings...", "Quit Firefox"]
        );

        // Case 2: GTK3 native `Application` column with its own `Quit` deduplicates `Quit {app_name}`
        let gtk3_root = make_node(
            0,
            "Root",
            MenuItemType::Submenu,
            None,
            vec![
                make_node(
                    10,
                    "Application",
                    MenuItemType::Submenu,
                    None,
                    vec![
                        make_node(11, "Preferences", MenuItemType::Standard, None, vec![]),
                        make_node(12, "Quit", MenuItemType::Standard, None, vec![]),
                    ],
                ),
                make_node(20, "View", MenuItemType::Submenu, None, vec![]),
            ],
        );

        let merged_gtk3 = MenuPipeline::apply_app_menu_template(
            gtk3_root,
            &template,
            &info,
            MenuSourceTier::GtkNative,
            "python3",
            Path::new("."),
            "Inkscape",
        );

        let gtk3_top: Vec<&str> = merged_gtk3
            .children
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(gtk3_top, vec!["Inkscape", "View"]);

        let gtk3_app_items: Vec<&str> = merged_gtk3.children[0]
            .children
            .iter()
            .filter(|n| n.item_type != MenuItemType::Separator)
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(
            gtk3_app_items,
            vec!["Preferences", "Quit", "Genubar Settings..."]
        );

        // Case 3: Normal app with no app column must not start with a leading separator
        let normal_root = make_node(
            0,
            "Root",
            MenuItemType::Submenu,
            None,
            vec![make_node(1, "File", MenuItemType::Submenu, None, vec![])],
        );
        let merged_normal = MenuPipeline::apply_app_menu_template(
            normal_root,
            &template,
            &info,
            MenuSourceTier::Gtk4Hook,
            "python3",
            Path::new("."),
            "Files",
        );
        assert_eq!(
            merged_normal.children[0].children[0].label,
            "Genubar Settings..."
        );
    }

    #[test]
    fn reconcile_app_name_columns_merges_or_expands_post_script_placeholders() {
        // Case 1: Post-template Python script prepends `{app_name}` next to already-expanded `Files`.
        let mut with_existing = make_node(
            0,
            "Root",
            MenuItemType::Submenu,
            None,
            vec![
                make_node(
                    -900,
                    "{app_name}",
                    MenuItemType::Submenu,
                    None,
                    vec![make_node(
                        -902,
                        "Copy PID (99)",
                        MenuItemType::Standard,
                        Some("copy:99"),
                        vec![],
                    )],
                ),
                make_node(
                    -10,
                    "Files",
                    MenuItemType::Submenu,
                    None,
                    vec![
                        make_node(
                            -11,
                            "Genubar Settings...",
                            MenuItemType::Standard,
                            Some("cmd:genubar-settings"),
                            vec![],
                        ),
                        make_node(
                            -12,
                            "Quit Files",
                            MenuItemType::Standard,
                            Some("sys:quit_app"),
                            vec![],
                        ),
                    ],
                ),
                make_node(1, "File", MenuItemType::Submenu, None, vec![]),
            ],
        );

        MandatoryMerger::reconcile_app_name_columns(&mut with_existing, "Files");

        let top_labels: Vec<&str> = with_existing
            .children
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(top_labels, vec!["Files", "File"]);
        let files_items: Vec<&str> = with_existing.children[0]
            .children
            .iter()
            .filter(|n| n.item_type != MenuItemType::Separator)
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(
            files_items,
            vec!["Copy PID (99)", "Genubar Settings...", "Quit Files"]
        );

        // Case 2: Standalone `{app_name}` column with no pre-existing `Files` column expands in place.
        let mut standalone = make_node(
            0,
            "Root",
            MenuItemType::Submenu,
            None,
            vec![make_node(
                -900,
                "{app_name}",
                MenuItemType::Submenu,
                None,
                vec![make_node(
                    -902,
                    "Custom Action",
                    MenuItemType::Standard,
                    Some("cmd:true"),
                    vec![],
                )],
            )],
        );
        MandatoryMerger::reconcile_app_name_columns(&mut standalone, "Files");
        assert_eq!(standalone.children.len(), 1);
        assert_eq!(standalone.children[0].label, "Files");
    }
}
