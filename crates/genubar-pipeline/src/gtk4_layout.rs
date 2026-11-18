use crate::gtk4_rules::{Gtk4ItemBucket, classify_gtk4_item};
use crate::python::{DEFAULT_SCRIPT_TIMEOUT_MS, PythonRunner};
use genubar_config::Gtk4LayoutAlgorithm;
use genubar_core::{ActiveWindowInfo, MenuItemType, MenuNode, MenuSourceTier};
use std::collections::HashSet;
use std::path::Path;

pub struct Gtk4LayoutOrganizer;

impl Gtk4LayoutOrganizer {
    pub fn apply(
        root: MenuNode,
        algorithm: Gtk4LayoutAlgorithm,
        expose_unknown: bool,
        script_path: Option<&Path>,
        python_bin: &str,
        config_dir: &Path,
        info: &ActiveWindowInfo,
    ) -> MenuNode {
        Self::apply_with_timeout(
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
    pub fn apply_with_timeout(
        root: MenuNode,
        algorithm: Gtk4LayoutAlgorithm,
        expose_unknown: bool,
        script_path: Option<&Path>,
        python_bin: &str,
        config_dir: &Path,
        info: &ActiveWindowInfo,
        timeout_ms: u64,
    ) -> MenuNode {
        match algorithm {
            Gtk4LayoutAlgorithm::Original => root,
            Gtk4LayoutAlgorithm::Standard => Self::organize_standard(root, expose_unknown),
            Gtk4LayoutAlgorithm::PythonScript => {
                if let Some(script) = script_path
                    && let Some(transformed) = PythonRunner::run_with_timeout(
                        python_bin,
                        config_dir,
                        script,
                        info,
                        MenuSourceTier::Gtk4Hook,
                        &root,
                        timeout_ms,
                    ) {
                        return transformed;
                    }
                root
            }
        }
    }

    fn normalize_col_label(label: &str) -> String {
        label
            .replace(['&', '_'], "")
            .trim()
            .to_ascii_lowercase()
    }

    fn is_already_structured_menubar(root: &MenuNode) -> bool {
        if root.children.len() < 3 {
            return false;
        }
        if !root
            .children
            .iter()
            .all(|col| col.item_type == MenuItemType::Submenu || !col.children.is_empty())
        {
            return false;
        }

        const STANDARD_COLUMNS: &[&str] = &[
            "file",
            "edit",
            "view",
            "help",
            "layer",
            "object",
            "path",
            "text",
            "filters",
            "extensions",
            "tools",
            "window",
            "bookmarks",
        ];

        let mut standard_hits = 0usize;
        for col in &root.children {
            let norm = Self::normalize_col_label(&col.label);
            if norm == "options" || norm == "menu" {
                return false;
            }
            if STANDARD_COLUMNS.contains(&norm.as_str()) {
                standard_hits += 1;
            }
        }
        standard_hits >= 2
    }

    fn organize_standard(root: MenuNode, expose_unknown: bool) -> MenuNode {
        if Self::is_already_structured_menubar(&root) {
            return root;
        }

        let flat_items: Vec<MenuNode> = root
            .children
            .iter()
            .flat_map(|col| col.children.iter().cloned())
            .filter(|item| item.item_type != MenuItemType::Separator)
            .collect();

        if flat_items.is_empty() {
            return root;
        }

        let mut scan_mode_items = Vec::new();
        let mut doc_type_items = Vec::new();
        let mut file_items = Vec::new();
        let mut edit_items = Vec::new();
        let mut view_items = Vec::new();
        let mut help_items = Vec::new();
        let mut seen_labels = HashSet::new();

        for item in flat_items {
            match classify_gtk4_item(&item, expose_unknown, &mut seen_labels) {
                Gtk4ItemBucket::ScanMode => scan_mode_items.push(item),
                Gtk4ItemBucket::DocumentType => doc_type_items.push(item),
                Gtk4ItemBucket::File => file_items.push(item),
                Gtk4ItemBucket::Edit => edit_items.push(item),
                Gtk4ItemBucket::View => view_items.push(item),
                Gtk4ItemBucket::Help => help_items.push(item),
                Gtk4ItemBucket::Skip => {}
            }
        }

        let mut col_id = -500;
        let mut prepended_file = Vec::new();
        for (title, sub_items) in [
            ("Scan Mode", scan_mode_items),
            ("Document Type", doc_type_items),
        ] {
            if !sub_items.is_empty() {
                prepended_file.push(MenuNode {
                    id: col_id,
                    label: String::from(title),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: sub_items,
                });
                col_id -= 1;
            }
        }
        if !prepended_file.is_empty() {
            prepended_file.extend(file_items);
            file_items = prepended_file;
        }

        let mut columns = Vec::new();
        for (title, items) in [
            ("File", file_items),
            ("Edit", edit_items),
            ("View", view_items),
            ("Help", help_items),
        ] {
            if !items.is_empty() {
                let separated_items = Self::insert_group_separators(items, &mut col_id);
                columns.push(MenuNode {
                    id: col_id,
                    label: title.to_string(),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: None,
                    children: separated_items,
                });
                col_id -= 1;
            }
        }

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
            children: columns,
        }
    }

    fn item_group_kind(item: &MenuNode) -> u8 {
        if item.item_type == MenuItemType::Submenu || !item.children.is_empty() {
            0
        } else if matches!(
            item.item_type,
            MenuItemType::Check(_) | MenuItemType::Radio(_)
        ) {
            1
        } else {
            2
        }
    }

    fn insert_group_separators(items: Vec<MenuNode>, next_sep_id: &mut i32) -> Vec<MenuNode> {
        let mut out = Vec::with_capacity(items.len());
        let mut prev_kind: Option<u8> = None;

        for item in items {
            if item.item_type == MenuItemType::Separator {
                continue;
            }
            let kind = Self::item_group_kind(&item);
            if let Some(pk) = prev_kind
                && pk != kind {
                    out.push(MenuNode {
                        id: *next_sep_id,
                        label: String::new(),
                        enabled: true,
                        visible: true,
                        item_type: MenuItemType::Separator,
                        shortcut: None,
                        action: None,
                        icon: None,
                        origin_path: None,
                        children: Vec::new(),
                    });
                    *next_sep_id -= 1;
                }
            prev_kind = Some(kind);
            out.push(item);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i32, label: &str, action: &str, item_type: MenuItemType) -> MenuNode {
        MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type,
            shortcut: None,
            action: Some(action.to_string()),
            icon: None,
            origin_path: None,
            children: vec![],
        }
    }

    #[test]
    fn gtk4_organizer_routes_labels_over_action_prefix_and_deduplicates() {
        let root = MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![MenuNode {
                id: 1,
                label: String::from("Menu"),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                icon: None,
                origin_path: None,
                children: vec![
                    item(2, "Copy Location", "view.copy-current-location", MenuItemType::Standard),
                    item(3, "Show Hidden Files", "view.show-hidden", MenuItemType::Check(false)),
                    item(4, "Show Hidden Files", "view.show-hidden", MenuItemType::Check(false)),
                    item(5, "About Files", "app.about", MenuItemType::Standard),
                ],
            }],
        };

        let organized = Gtk4LayoutOrganizer::apply(
            root,
            Gtk4LayoutAlgorithm::Standard,
            false,
            None,
            "python3",
            Path::new("/tmp"),
            &ActiveWindowInfo::default(),
        );

        let col_labels: Vec<&str> = organized.children.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(col_labels, vec!["File", "View", "Help"]);
        assert_eq!(organized.children[0].children[0].label, "Copy Location");
        assert_eq!(organized.children[1].children.len(), 1);
        assert_eq!(organized.children[2].children[0].label, "About Files");
    }

    #[test]
    fn gtk4_organizer_preserves_structured_multicolumn_menubar() {
        let col = |id: i32, label: &str, child_label: &str| MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![item(id * 10, child_label, "doc.action", MenuItemType::Standard)],
        };

        let root = MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![
                col(1, "_File", "Save"),
                col(2, "_Edit", "Undo"),
                col(3, "_View", "Zoom In"),
                col(4, "_Layer", "Add Layer"),
                col(5, "_Object", "Group"),
                col(6, "_Help", "Inkscape Manual"),
            ],
        };

        let organized = Gtk4LayoutOrganizer::apply(
            root,
            Gtk4LayoutAlgorithm::Standard,
            false,
            None,
            "python3",
            Path::new("/tmp"),
            &ActiveWindowInfo::default(),
        );

        let col_labels: Vec<&str> = organized.children.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            col_labels,
            vec!["_File", "_Edit", "_View", "_Layer", "_Object", "_Help"]
        );
    }
}
