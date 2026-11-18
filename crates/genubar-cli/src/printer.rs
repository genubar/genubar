use genubar_core::{MenuItemType, MenuNode};

fn format_node_line(node: &MenuNode, depth: usize) -> Option<String> {
    if depth == 0 || !node.visible {
        return None;
    }

    let indent = "  ".repeat(depth);
    Some(match &node.item_type {
        MenuItemType::Separator => format!("{indent}├── ─────────────── [id: {}]", node.id),
        other => {
            let state_badge = if !node.enabled { " (disabled)" } else { "" };
            let type_badge = match other {
                MenuItemType::Check(true) => "[x] ",
                MenuItemType::Check(false) => "[ ] ",
                MenuItemType::Radio(true) => "(*) ",
                MenuItemType::Radio(false) => "( ) ",
                _ => "",
            };
            let shortcut_badge = node
                .shortcut
                .as_ref()
                .map(|s| format!(" <{}>", s.join("+")))
                .unwrap_or_default();
            let action_badge = node
                .action
                .as_ref()
                .map(|a| format!(" [{a}]"))
                .unwrap_or_default();

            let clean_label = node.clean_label();
            format!(
                "{indent}├── [{id}] {type_badge}{clean_label}{shortcut_badge}{action_badge}{state_badge}",
                id = node.id
            )
        }
    })
}

pub fn print_tree(node: &MenuNode, depth: usize) {
    if let Some(line) = format_node_line(node, depth) {
        println!("{line}");
    }

    for child in &node.children {
        print_tree(child, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_node_lines_with_clean_mnemonics_shortcuts_and_states() {
        let node = MenuNode {
            id: 7,
            label: String::from("_Save As..."),
            enabled: false,
            visible: true,
            item_type: MenuItemType::Check(true),
            shortcut: Some(vec![
                String::from("Ctrl"),
                String::from("Shift"),
                String::from("S"),
            ]),
            action: Some(String::from("win.save-as")),
            icon: None,
            origin_path: None,
            children: vec![],
        };

        assert_eq!(format_node_line(&node, 0), None);
        assert_eq!(
            format_node_line(&node, 1).as_deref(),
            Some("  ├── [7] [x] Save As... <Ctrl+Shift+S> [win.save-as] (disabled)")
        );
    }
}
