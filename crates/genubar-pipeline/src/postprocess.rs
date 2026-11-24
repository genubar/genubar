use genubar_core::{MenuItemType, MenuNode};

pub fn sanitize_separators(node: &mut MenuNode) {
    for child in &mut node.children {
        sanitize_separators(child);
    }

    let mut cleaned: Vec<MenuNode> = Vec::with_capacity(node.children.len());
    for item in node.children.drain(..) {
        if !item.visible {
            continue;
        }
        if item.item_type == MenuItemType::Separator {
            let is_at_start = cleaned.is_empty();
            let prev_is_sep = cleaned
                .last()
                .map(|prev| prev.item_type == MenuItemType::Separator)
                .unwrap_or(false);
            if is_at_start || prev_is_sep {
                continue;
            }
        }
        cleaned.push(item);
    }

    while cleaned
        .last()
        .map(|last| last.item_type == MenuItemType::Separator)
        .unwrap_or(false)
    {
        cleaned.pop();
    }

    node.children = cleaned;
}

pub fn apply_more_overflow(root: &mut MenuNode, max_columns: usize, more_label: &str) {
    if max_columns == 0 || root.children.len() <= max_columns {
        return;
    }
    let split_idx = max_columns.saturating_sub(1);
    let overflow_cols = root.children.split_off(split_idx);
    root.children.push(MenuNode {
        id: -880,
        label: more_label.to_string(),
        enabled: true,
        visible: true,
        item_type: MenuItemType::Submenu,
        shortcut: None,
        action: None,
        icon: None,
        origin_path: None,
        children: overflow_cols,
    });
}

fn retain_nodes_recursive(node: &mut MenuNode, predicate: &impl Fn(&MenuNode) -> bool) {
    node.children.retain(|child| predicate(child));
    for child in &mut node.children {
        retain_nodes_recursive(child, predicate);
    }
}

/// Removes inline `SearchBar` nodes when global menu search is disabled.
pub fn strip_search_bars(root: &mut MenuNode) {
    retain_nodes_recursive(root, &|n| !matches!(n.item_type, MenuItemType::SearchBar));
}

/// Removes action nodes matching `action_name` recursively across the menu tree.
pub fn strip_action_items(root: &mut MenuNode, action_name: &str) {
    retain_nodes_recursive(root, &|n| n.action.as_deref() != Some(action_name));
}

/// Clears keyboard shortcut hints recursively across the menu tree.
pub fn strip_shortcuts(root: &mut MenuNode) {
    root.shortcut = None;
    for child in &mut root.children {
        strip_shortcuts(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: i32, kind: MenuItemType, action: Option<&str>) -> MenuNode {
        MenuNode {
            id,
            label: format!("Item {id}"),
            enabled: true,
            visible: true,
            item_type: kind,
            shortcut: None,
            action: action.map(str::to_string),
            icon: None,
            origin_path: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn strip_helpers_remove_search_bars_and_targeted_actions() {
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
            children: vec![MenuNode {
                id: 1,
                label: String::from("Help"),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                icon: None,
                origin_path: None,
                children: vec![
                    leaf(2, MenuItemType::SearchBar, None),
                    leaf(3, MenuItemType::Standard, Some("sys:refresh_menu")),
                    leaf(4, MenuItemType::Standard, Some("win.about")),
                ],
            }],
        };

        strip_search_bars(&mut root);
        strip_action_items(&mut root, "sys:refresh_menu");
        assert_eq!(root.children[0].children.len(), 1);
        assert_eq!(root.children[0].children[0].id, 4);
    }

    #[test]
    fn strip_shortcuts_clears_nested_shortcuts_while_preserving_items() {
        let mut item = leaf(4, MenuItemType::Standard, Some("win.about"));
        item.shortcut = Some(vec![String::from("Ctrl"), String::from("I")]);
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
            children: vec![MenuNode {
                id: 1,
                label: String::from("Help"),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: Some(vec![String::from("F1")]),
                action: None,
                icon: None,
                origin_path: None,
                children: vec![item],
            }],
        };

        strip_shortcuts(&mut root);
        assert_eq!(root.children[0].shortcut, None);
        assert_eq!(root.children[0].children.len(), 1);
        assert_eq!(root.children[0].children[0].shortcut, None);
        assert_eq!(
            root.children[0].children[0].action.as_deref(),
            Some("win.about")
        );
    }
}
