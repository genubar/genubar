use crate::gtk4_layout::Gtk4LayoutOrganizer;
use crate::icons::IconResolver;
use crate::inherit::InheritanceEngine;
use crate::mandatory::MandatoryMerger;
use crate::postprocess::{apply_more_overflow, sanitize_separators, strip_action_items, strip_search_bars};
use crate::python::{DEFAULT_SCRIPT_TIMEOUT_MS, PythonRunner};
use crate::sorter::RootMenuSorter;
use crate::static_builder::StaticMenuBuilder;
use genubar_config::{
    AppMenuTemplateConfig, AppProfile, AppTemplateMode, Gtk4LayoutAlgorithm, IconPackManifest,
    IconsConfig, MenuNodeDef, RootSortConfig,
};
use genubar_core::{ActiveWindowInfo, MenuItemType, MenuNode, MenuSourceTier, clean_label};
use std::collections::HashMap;
use std::path::Path;

/// Runtime toggles propagated from `GeneralConfig` into menu transformation stages.
#[derive(Debug, Clone, Copy)]
pub struct PipelineRuntimeConfig {
    pub script_timeout_ms: u64,
    pub show_menu_icons: bool,
    pub auto_map_standard_icons: bool,
    pub enable_menu_search: bool,
}

impl Default for PipelineRuntimeConfig {
    fn default() -> Self {
        Self {
            script_timeout_ms: DEFAULT_SCRIPT_TIMEOUT_MS,
            show_menu_icons: true,
            auto_map_standard_icons: true,
            enable_menu_search: true,
        }
    }
}

pub struct MenuPipeline;

impl MenuPipeline {
    pub fn sanitize_separators(node: &mut MenuNode) {
        sanitize_separators(node);
    }

    pub fn apply_profile(
        live_root: MenuNode,
        profile: &AppProfile,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        python_bin: &str,
        config_dir: &Path,
        app_name: &str,
    ) -> MenuNode {
        Self::apply_profile_with_config(
            live_root,
            profile,
            info,
            tier,
            python_bin,
            config_dir,
            app_name,
            PipelineRuntimeConfig::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_profile_with_config(
        live_root: MenuNode,
        profile: &AppProfile,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        python_bin: &str,
        config_dir: &Path,
        app_name: &str,
        runtime: PipelineRuntimeConfig,
    ) -> MenuNode {
        let mut live_index = HashMap::new();
        let mut path_buf = Vec::new();
        InheritanceEngine::index_live_nodes(&live_root.children, &mut path_buf, &mut live_index);

        let mut next_id = -400;
        let children = if profile.columns.is_empty() {
            live_root.children
        } else {
            InheritanceEngine::evaluate_defs_against_live(
                &profile.columns,
                &live_root.children,
                &live_index,
                profile.sync_new_items,
                Some(app_name),
                tier,
                &mut next_id,
            )
        };

        let mut root = MenuNode {
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
        };

        if let Some(ref script_rel) = profile.python_script
            && let Some(script_out) = PythonRunner::run_with_timeout(
                python_bin,
                config_dir,
                script_rel,
                info,
                tier,
                &root,
                runtime.script_timeout_ms,
            ) {
                root = script_out;
            }

        sanitize_separators(&mut root);
        root
    }

    pub fn apply_profile_to_fetched_submenu(
        mut fetched_submenu: MenuNode,
        profile: &AppProfile,
        app_name: &str,
        tier: MenuSourceTier,
    ) -> MenuNode {
        let sub_norm = fetched_submenu.clean_label().to_lowercase();
        if sub_norm.is_empty() {
            sanitize_separators(&mut fetched_submenu);
            return fetched_submenu;
        }

        for col_def in &profile.columns {
            if let MenuNodeDef::Inherit {
                from,
                match_labels,
                sync_new_items,
                items: Some(sub_defs),
                ..
            } = col_def
            {
                let matches_col = (!from.is_empty() && clean_label(from).to_lowercase() == sub_norm)
                    || match_labels
                        .iter()
                        .any(|l| clean_label(l).to_lowercase() == sub_norm);

                if matches_col {
                    Self::evaluate_fetched_submenu_defs(
                        &mut fetched_submenu,
                        sub_defs,
                        *sync_new_items,
                        None,
                        app_name,
                        tier,
                        -650,
                    );
                    break;
                }
            }
        }

        sanitize_separators(&mut fetched_submenu);
        fetched_submenu
    }

    /// Merges mandatory `app_menu.toml` entries (such as `SearchBar` in Help or text transforms in Edit)
    /// into lazily fetched top-level submenus resolved via `FetchSubmenu`.
    pub fn apply_app_menu_to_fetched_submenu(
        mut fetched_submenu: MenuNode,
        template: &AppMenuTemplateConfig,
        app_name: &str,
        tier: MenuSourceTier,
    ) -> MenuNode {
        let sub_norm = fetched_submenu.clean_label().to_lowercase();
        if sub_norm.is_empty() {
            sanitize_separators(&mut fetched_submenu);
            return fetched_submenu;
        }

        for col_def in &template.columns {
            if let MenuNodeDef::Submenu {
                label,
                visible: true,
                only_tiers,
                merge_with_live: true,
                match_aliases,
                items,
                ..
            } = col_def
            {
                if !StaticMenuBuilder::tier_matches(only_tiers, tier) {
                    continue;
                }
                let expanded_label = StaticMenuBuilder::expand_label(label, Some(app_name));
                let candidate_names =
                    MandatoryMerger::collect_candidate_names(&expanded_label, match_aliases);

                if candidate_names.contains(&sub_norm) {
                    let mut initial_index = HashMap::new();
                    let mut path_buf = vec![sub_norm.clone()];
                    InheritanceEngine::index_live_nodes(
                        &fetched_submenu.children,
                        &mut path_buf,
                        &mut initial_index,
                    );
                    let mut root_path_buf = Vec::new();
                    InheritanceEngine::index_live_nodes(
                        &fetched_submenu.children,
                        &mut root_path_buf,
                        &mut initial_index,
                    );

                    let filtered_defs = MandatoryMerger::filter_unclaimed_action_defs(
                        items,
                        &expanded_label,
                        &initial_index,
                    );

                    Self::evaluate_fetched_submenu_defs(
                        &mut fetched_submenu,
                        &filtered_defs,
                        false,
                        Some(&sub_norm),
                        app_name,
                        tier,
                        -750,
                    );
                    break;
                }
            }
        }

        sanitize_separators(&mut fetched_submenu);
        fetched_submenu
    }

    fn evaluate_fetched_submenu_defs(
        fetched_submenu: &mut MenuNode,
        defs: &[MenuNodeDef],
        sync_new_items: bool,
        scoped_parent_norm: Option<&str>,
        app_name: &str,
        tier: MenuSourceTier,
        start_id: i32,
    ) {
        let mut live_index = HashMap::new();
        if let Some(parent) = scoped_parent_norm {
            let mut path_buf = vec![parent.to_string()];
            InheritanceEngine::index_live_nodes(
                &fetched_submenu.children,
                &mut path_buf,
                &mut live_index,
            );
        }
        let mut root_path_buf = Vec::new();
        InheritanceEngine::index_live_nodes(
            &fetched_submenu.children,
            &mut root_path_buf,
            &mut live_index,
        );

        let mut next_id = start_id;
        fetched_submenu.children = InheritanceEngine::evaluate_defs_against_live(
            defs,
            &fetched_submenu.children,
            &live_index,
            sync_new_items,
            Some(app_name),
            tier,
            &mut next_id,
        );
    }

    pub fn apply_app_menu_template(
        app_columns_root: MenuNode,
        template: &AppMenuTemplateConfig,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        python_bin: &str,
        config_dir: &Path,
        app_name: &str,
    ) -> MenuNode {
        Self::apply_app_menu_template_with_config(
            app_columns_root,
            template,
            info,
            tier,
            python_bin,
            config_dir,
            app_name,
            PipelineRuntimeConfig::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_app_menu_template_with_config(
        app_columns_root: MenuNode,
        template: &AppMenuTemplateConfig,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        python_bin: &str,
        config_dir: &Path,
        app_name: &str,
        runtime: PipelineRuntimeConfig,
    ) -> MenuNode {
        let mut live_index = HashMap::new();
        let mut path_buf = Vec::new();
        InheritanceEngine::index_live_nodes(
            &app_columns_root.children,
            &mut path_buf,
            &mut live_index,
        );

        let mut next_id = -10;
        let children = InheritanceEngine::evaluate_defs_against_live(
            &template.columns,
            &app_columns_root.children,
            &live_index,
            template.sync_new_items,
            Some(app_name),
            tier,
            &mut next_id,
        );

        let mut root = MenuNode {
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
        };

        if template.mode == AppTemplateMode::PythonScript
            && let Some(ref script) = template.python_script
                && let Some(out) = PythonRunner::run_with_timeout(
                    python_bin,
                    config_dir,
                    script,
                    info,
                    tier,
                    &root,
                    runtime.script_timeout_ms,
                ) {
                    root = out;
                }

        sanitize_separators(&mut root);
        root
    }

    pub fn apply_more_overflow(root: &mut MenuNode, max_columns: usize, more_label: &str) {
        apply_more_overflow(root, max_columns, more_label);
    }

    pub fn apply_gtk4_layout(
        root: MenuNode,
        algorithm: Gtk4LayoutAlgorithm,
        expose_unknown: bool,
        script_path: Option<&Path>,
        python_bin: &str,
        config_dir: &Path,
        info: &ActiveWindowInfo,
    ) -> MenuNode {
        Self::apply_gtk4_layout_with_timeout(
            root,
            algorithm,
            expose_unknown,
            script_path,
            python_bin,
            config_dir,
            info,
            DEFAULT_SCRIPT_TIMEOUT_MS,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_gtk4_layout_with_timeout(
        root: MenuNode,
        algorithm: Gtk4LayoutAlgorithm,
        expose_unknown: bool,
        script_path: Option<&Path>,
        python_bin: &str,
        config_dir: &Path,
        info: &ActiveWindowInfo,
        timeout_ms: u64,
    ) -> MenuNode {
        let mut organized = Gtk4LayoutOrganizer::apply_with_timeout(
            root,
            algorithm,
            expose_unknown,
            script_path,
            python_bin,
            config_dir,
            info,
            timeout_ms,
        );
        sanitize_separators(&mut organized);
        organized
    }

    pub fn apply_root_sorting(
        root: MenuNode,
        sort_cfg: &RootSortConfig,
        info: &ActiveWindowInfo,
        app_name: &str,
        python_bin: &str,
        config_dir: &Path,
    ) -> MenuNode {
        Self::apply_root_sorting_with_timeout(
            root,
            sort_cfg,
            info,
            app_name,
            python_bin,
            config_dir,
            DEFAULT_SCRIPT_TIMEOUT_MS,
        )
    }

    pub fn apply_root_sorting_with_timeout(
        root: MenuNode,
        sort_cfg: &RootSortConfig,
        info: &ActiveWindowInfo,
        app_name: &str,
        python_bin: &str,
        config_dir: &Path,
        timeout_ms: u64,
    ) -> MenuNode {
        RootMenuSorter::apply_with_timeout(
            root, sort_cfg, info, app_name, python_bin, config_dir, timeout_ms,
        )
    }

    pub fn apply_menu_icons(
        root: &mut MenuNode,
        info: &ActiveWindowInfo,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
    ) {
        IconResolver::apply_icons(root, info, icons_cfg, active_packs);
    }

    pub fn apply_menu_icons_with_config(
        root: &mut MenuNode,
        info: &ActiveWindowInfo,
        icons_cfg: &IconsConfig,
        active_packs: &[IconPackManifest],
        runtime: PipelineRuntimeConfig,
    ) {
        IconResolver::apply_icons_with_options(
            root,
            info,
            icons_cfg,
            active_packs,
            runtime.show_menu_icons,
            runtime.auto_map_standard_icons,
        );
    }

    pub fn apply_runtime_filters(
        root: &mut MenuNode,
        runtime: PipelineRuntimeConfig,
        strip_refresh_button: bool,
    ) {
        if !runtime.enable_menu_search {
            strip_search_bars(root);
        }
        if strip_refresh_button {
            strip_action_items(root, "sys:refresh_menu");
        }
        sanitize_separators(root);
    }

    pub fn run_python_script(
        python_bin: &str,
        config_dir: &Path,
        script_path: &Path,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        root: &MenuNode,
    ) -> Option<MenuNode> {
        PythonRunner::run(python_bin, config_dir, script_path, info, tier, root)
    }

    pub fn run_python_script_with_timeout(
        python_bin: &str,
        config_dir: &Path,
        script_path: &Path,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        root: &MenuNode,
        timeout_ms: u64,
    ) -> Option<MenuNode> {
        PythonRunner::run_with_timeout(
            python_bin, config_dir, script_path, info, tier, root, timeout_ms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_config::AppTemplateMode;

    fn node(id: i32, label: &str, item_type: MenuItemType, children: Vec<MenuNode>) -> MenuNode {
        MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children,
        }
    }

    #[test]
    fn fetched_submenu_merges_profile_overrides_and_app_menu_without_duplicate_actions() {
        let fetched = node(
            10,
            "_Help",
            MenuItemType::Submenu,
            vec![
                node(11, "User _Guide", MenuItemType::Standard, vec![]),
                node(12, "", MenuItemType::Separator, vec![]),
                node(13, "", MenuItemType::Separator, vec![]),
                node(14, "About App", MenuItemType::Standard, vec![]),
            ],
        );

        let profile = AppProfile {
            columns: vec![MenuNodeDef::Inherit {
                from: String::from("Help"),
                match_labels: vec![],
                match_aliases: vec![],
                origin_path: None,
                label: None,
                icon: None,
                enabled: true,
                visible: true,
                shortcut: None,
                sync_new_items: true,
                items: Some(vec![MenuNodeDef::Inherit {
                    from: String::from("About App"),
                    match_labels: vec![],
                    match_aliases: vec![],
                    origin_path: None,
                    label: Some(String::from("About {app_name}")),
                    icon: None,
                    enabled: true,
                    visible: true,
                    shortcut: None,
                    sync_new_items: true,
                    items: None,
                }]),
            }],
            ..Default::default()
        };

        let after_profile = MenuPipeline::apply_profile_to_fetched_submenu(
            fetched,
            &profile,
            "MyApp",
            MenuSourceTier::CanonicalDbus,
        );
        let prof_labels: Vec<&str> = after_profile
            .children
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(prof_labels, vec!["About MyApp", "User _Guide"]);

        let template = AppMenuTemplateConfig {
            mode: AppTemplateMode::Static,
            sync_new_items: true,
            python_script: None,
            columns: vec![MenuNodeDef::Submenu {
                label: String::from("Help"),
                icon: None,
                enabled: true,
                visible: true,
                only_tiers: None,
                merge_with_live: true,
                match_aliases: vec![],
                items: vec![
                    MenuNodeDef::SearchBar {
                        placeholder: String::from("Find action..."),
                        visible: true,
                        only_tiers: None,
                    },
                    MenuNodeDef::Action {
                        label: String::from("User Guide"),
                        action: Some(String::from("custom.guide")),
                        icon: None,
                        enabled: true,
                        visible: true,
                        shortcut: None,
                        only_tiers: None,
                        match_aliases: vec![],
                    },
                    MenuNodeDef::InheritOthers,
                ],
            }],
        };

        let after_app = MenuPipeline::apply_app_menu_to_fetched_submenu(
            after_profile,
            &template,
            "MyApp",
            MenuSourceTier::CanonicalDbus,
        );
        assert!(matches!(after_app.children[0].item_type, MenuItemType::SearchBar));
        assert_eq!(after_app.children.len(), 3);
    }

    #[test]
    fn more_overflow_splits_excess_columns_and_sanitizes_edges() {
        let mut root = node(
            0,
            "Root",
            MenuItemType::Submenu,
            vec![
                node(1, "App", MenuItemType::Submenu, vec![]),
                node(2, "File", MenuItemType::Submenu, vec![]),
                node(3, "Edit", MenuItemType::Submenu, vec![]),
                node(4, "View", MenuItemType::Submenu, vec![]),
            ],
        );

        MenuPipeline::apply_more_overflow(&mut root, 3, "More Tools");
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.children[2].label, "More Tools");
        assert_eq!(root.children[2].children.len(), 2);
    }
}
