use crate::python::{DEFAULT_SCRIPT_TIMEOUT_MS, PythonRunner};
use genubar_config::{RootSortConfig, RootSortMode};
use genubar_core::{ActiveWindowInfo, MenuNode, MenuSourceTier, clean_label, wildcard_match};
use std::path::Path;

const STANDARD_ORDER: &[&str] = &[
    "{app_name}",
    "file",
    "edit",
    "selection",
    "view",
    "go",
    "navigate",
    "code",
    "refactor",
    "build",
    "run",
    "*",
    "tools",
    "window",
    "windows",
    "help",
];

pub struct RootMenuSorter;

impl RootMenuSorter {
    pub fn apply(
        root: MenuNode,
        sort_cfg: &RootSortConfig,
        info: &ActiveWindowInfo,
        app_name: &str,
        python_bin: &str,
        config_dir: &Path,
    ) -> MenuNode {
        Self::apply_with_timeout(
            root,
            sort_cfg,
            info,
            app_name,
            python_bin,
            config_dir,
            DEFAULT_SCRIPT_TIMEOUT_MS,
        )
    }

    pub fn apply_with_timeout(
        mut root: MenuNode,
        sort_cfg: &RootSortConfig,
        info: &ActiveWindowInfo,
        app_name: &str,
        python_bin: &str,
        config_dir: &Path,
        timeout_ms: u64,
    ) -> MenuNode {
        if sort_cfg.mode == RootSortMode::Disabled
            || root.children.len() <= 2
            || Self::is_app_excluded(&sort_cfg.exclude_apps, info)
        {
            return root;
        }

        match sort_cfg.mode {
            RootSortMode::Disabled => root,
            RootSortMode::Standard => {
                let order: Vec<String> = STANDARD_ORDER.iter().map(|s| (*s).to_string()).collect();
                root.children = Self::sort_columns_by_pattern(root.children, &order, app_name);
                root
            }
            RootSortMode::Custom => {
                if !sort_cfg.custom_order.is_empty() {
                    root.children = Self::sort_columns_by_pattern(
                        root.children,
                        &sort_cfg.custom_order,
                        app_name,
                    );
                }
                root
            }
            RootSortMode::PythonScript => {
                if let Some(ref script) = sort_cfg.python_script
                    && let Some(sorted) = PythonRunner::run_with_timeout(
                        python_bin,
                        config_dir,
                        script,
                        info,
                        MenuSourceTier::AppProfile,
                        &root,
                        timeout_ms,
                    )
                {
                    return sorted;
                }
                root
            }
        }
    }

    fn is_app_excluded(exclude_apps: &[String], info: &ActiveWindowInfo) -> bool {
        let app_id_low = info.app_id.to_lowercase();
        let title_low = info.title.to_lowercase();
        exclude_apps.iter().any(|ex| {
            let pat = ex.trim();
            if pat.is_empty() {
                return false;
            }
            if pat.contains('*') {
                wildcard_match(pat, &info.app_id) || wildcard_match(pat, &info.title)
            } else {
                let pat_low = pat.to_lowercase();
                app_id_low.contains(&pat_low) || title_low.contains(&pat_low)
            }
        })
    }

    fn sort_columns_by_pattern(
        columns: Vec<MenuNode>,
        pattern: &[String],
        app_name: &str,
    ) -> Vec<MenuNode> {
        let app_norm = clean_label(app_name).to_lowercase();
        let expanded_pattern: Vec<String> = pattern
            .iter()
            .map(|p| {
                if p == "{app_name}" {
                    app_norm.clone()
                } else {
                    clean_label(p).to_lowercase()
                }
            })
            .collect();

        let wildcard_pos = expanded_pattern.iter().position(|p| p == "*");
        let mut buckets: Vec<Vec<MenuNode>> = vec![Vec::new(); expanded_pattern.len()];
        let mut unmatched_others: Vec<MenuNode> = Vec::new();

        for (idx, col) in columns.into_iter().enumerate() {
            let col_norm = col.clean_label().to_lowercase();
            if idx == 0 && expanded_pattern.first() == Some(&app_norm) {
                buckets[0].push(col);
                continue;
            }
            if let Some(pos) = expanded_pattern
                .iter()
                .position(|p| p != "*" && *p == col_norm)
            {
                buckets[pos].push(col);
            } else {
                unmatched_others.push(col);
            }
        }

        let mut out = Vec::new();
        for (i, bucket) in buckets.into_iter().enumerate() {
            if Some(i) == wildcard_pos {
                out.append(&mut unmatched_others);
            } else {
                out.extend(bucket);
            }
        }

        if wildcard_pos.is_none() {
            out.append(&mut unmatched_others);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_core::MenuItemType;

    fn col(id: i32, label: &str) -> MenuNode {
        MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![],
        }
    }

    fn root_with(labels: &[&str]) -> MenuNode {
        MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Standard,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: labels
                .iter()
                .enumerate()
                .map(|(i, l)| col((i + 1) as i32, l))
                .collect(),
        }
    }

    #[test]
    fn sorts_standard_custom_disabled_and_excluded_apps() {
        let info = ActiveWindowInfo {
            app_id: String::from("org.gnome.Ptyxis"),
            title: String::from("Terminal"),
            ..Default::default()
        };
        let root = root_with(&["MyBrand", "_Help", "_Plugins", "_Edit", "_File"]);

        let standard_cfg = RootSortConfig {
            mode: RootSortMode::Standard,
            exclude_apps: vec![],
            custom_order: vec![],
            python_script: None,
        };
        let sorted_std = RootMenuSorter::apply_with_timeout(
            root.clone(),
            &standard_cfg,
            &info,
            "Ptyxis",
            "python3",
            Path::new("/tmp"),
            500,
        );
        let std_labels: Vec<String> = sorted_std
            .children
            .iter()
            .map(MenuNode::clean_label)
            .collect();
        assert_eq!(
            std_labels,
            vec!["MyBrand", "File", "Edit", "Plugins", "Help"]
        );

        let custom_cfg = RootSortConfig {
            mode: RootSortMode::Custom,
            exclude_apps: vec![],
            custom_order: vec![
                String::from("{app_name}"),
                String::from("Help"),
                String::from("File"),
                String::from("*"),
            ],
            python_script: None,
        };
        let sorted_custom = RootMenuSorter::apply_with_timeout(
            root.clone(),
            &custom_cfg,
            &info,
            "Ptyxis",
            "python3",
            Path::new("/tmp"),
            500,
        );
        let custom_labels: Vec<String> = sorted_custom
            .children
            .iter()
            .map(MenuNode::clean_label)
            .collect();
        assert_eq!(
            custom_labels,
            vec!["MyBrand", "Help", "File", "Plugins", "Edit"]
        );

        let excluded_cfg = RootSortConfig {
            mode: RootSortMode::Standard,
            exclude_apps: vec![String::from("*ptyxis*")],
            custom_order: vec![],
            python_script: None,
        };
        let excluded_out = RootMenuSorter::apply_with_timeout(
            root.clone(),
            &excluded_cfg,
            &info,
            "Ptyxis",
            "python3",
            Path::new("/tmp"),
            500,
        );
        let excluded_labels: Vec<String> = excluded_out
            .children
            .iter()
            .map(MenuNode::clean_label)
            .collect();
        assert_eq!(
            excluded_labels,
            vec!["MyBrand", "Help", "Plugins", "Edit", "File"]
        );
    }
}
