use serde::{Deserialize, Serialize};

/// Strips mnemonic underscores (`'_'`) and trims surrounding whitespace from a menu label.
pub fn clean_label(raw: &str) -> String {
    raw.replace('_', "").trim().to_string()
}

/// Case-insensitive wildcard matcher supporting exact, prefix (`"text*"`), suffix (`"*text"`),
/// substring (`"*text*"`), and multi-wildcard (`"a*b*c"`) patterns.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pat = clean_label(pattern).to_lowercase();
    let txt = clean_label(text).to_lowercase();

    if pat.is_empty() {
        return false;
    }
    if pat == "*" {
        return true;
    }
    if !pat.contains('*') {
        return pat == txt;
    }

    let starts_wild = pat.starts_with('*');
    let ends_wild = pat.ends_with('*');
    let parts: Vec<&str> = pat.split('*').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return true;
    }

    let mut current_idx = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 && !starts_wild {
            if !txt.starts_with(part) {
                return false;
            }
            current_idx = part.len();
        } else if i == parts.len() - 1 && !ends_wild {
            if !txt[current_idx..].ends_with(part) {
                return false;
            }
            current_idx = txt.len();
        } else if let Some(found_offset) = txt[current_idx..].find(part) {
            current_idx += found_offset + part.len();
        } else {
            return false;
        }
    }

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuSourceTier {
    DesktopIdle,
    GtkNative,
    Gtk4Hook,
    CanonicalDbus,
    Atspi,
    AppProfile,
    AppFallback,
}

impl MenuSourceTier {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::DesktopIdle => "Desktop-Idle",
            Self::GtkNative => "GTK-Native",
            Self::Gtk4Hook => "GTK4-Hook",
            Self::CanonicalDbus => "Canonical-DBusMenu",
            Self::Atspi => "AT-SPI2",
            Self::AppProfile => "App-Profile",
            Self::AppFallback => "Default-Fallback",
        }
    }
}

/// Discriminant for menu row rendering and activation behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuItemType {
    Standard,
    Separator,
    SearchBar,
    WindowList { min_windows_to_show: usize, max_title_length: usize },
    Check(bool),
    Radio(bool),
    Submenu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuNode {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub item_type: MenuItemType,
    pub shortcut: Option<Vec<String>>,
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_path: Option<Vec<String>>,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    pub fn clean_label(&self) -> String {
        clean_label(&self.label)
    }

    pub fn find_by_id(&self, target_id: i32) -> Option<&MenuNode> {
        if self.id == target_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(target_id) {
                return Some(found);
            }
        }
        None
    }

    /// Attaches lazily fetched submenu children into the active `MenuNode` tree by parent ID.
    pub fn attach_submenu_children(&mut self, submenu_id: i32, children: Vec<MenuNode>) -> bool {
        if self.id == submenu_id {
            self.children = children;
            return true;
        }
        for child in &mut self.children {
            if child.attach_submenu_children(submenu_id, children.clone()) {
                return true;
            }
        }
        false
    }

    /// Toggles a `Check` item or updates a mutually exclusive `Radio` group by `item_id`.
    pub fn toggle_check_or_radio_by_id(&mut self, target_id: i32) -> bool {
        Self::toggle_in_slice_by_id(&mut self.children, target_id)
    }

    fn toggle_in_slice_by_id(nodes: &mut [MenuNode], target_id: i32) -> bool {
        if let Some(pos) = nodes.iter().position(|n| n.id == target_id) {
            return match nodes[pos].item_type {
                MenuItemType::Check(curr) => {
                    nodes[pos].item_type = MenuItemType::Check(!curr);
                    true
                }
                MenuItemType::Radio(_) => {
                    Self::update_radio_group_at_index(nodes, pos);
                    true
                }
                _ => false,
            }
        }

        for node in nodes.iter_mut() {
            if Self::toggle_in_slice_by_id(&mut node.children, target_id) {
                return true;
            }
        }
        false
    }

    /// Toggles a `Check` item or `Radio` group by relative label path, optionally syncing live states.
    pub fn toggle_check_or_radio_by_path(
        nodes: &mut [MenuNode],
        rel_path: &[String],
        live_states: Option<&[(String, bool)]>,
    ) -> bool {
        let Some((first, rest)) = rel_path.split_first() else {
            return false;
        };
        let target_clean = clean_label(first).to_lowercase();

        let Some(pos) = nodes
            .iter()
            .position(|n| n.clean_label().to_lowercase() == target_clean)
        else {
            return false;
        };

        if rest.is_empty() {
            return match nodes[pos].item_type {
                MenuItemType::Check(old_checked) => {
                    let new_checked = live_states
                        .and_then(|list| {
                            list.iter()
                                .find(|(lbl, _)| clean_label(lbl).to_lowercase() == target_clean)
                                .map(|(_, st)| *st)
                        })
                        .filter(|&live_val| live_val != old_checked)
                        .unwrap_or(!old_checked);

                    nodes[pos].item_type = MenuItemType::Check(new_checked);
                    true
                }
                MenuItemType::Radio(old_selected) => {
                    let live_changed = live_states
                        .and_then(|list| {
                            list.iter()
                                .find(|(lbl, _)| clean_label(lbl).to_lowercase() == target_clean)
                                .map(|(_, st)| *st)
                        })
                        .map(|live_val| live_val && !old_selected)
                        .unwrap_or(false);

                    if live_changed && let Some(list) = live_states {
                        Self::apply_live_radio_states(nodes, pos, list);
                        return true;
                    }
                    Self::update_radio_group_at_index(nodes, pos);
                    true
                }
                _ => false,
            }
        }

        Self::toggle_check_or_radio_by_path(&mut nodes[pos].children, rest, live_states)
    }

    // Radio groups are bounded by surrounding separators so unrelated radio sections stay independent.
    fn update_radio_group_at_index(nodes: &mut [MenuNode], clicked_idx: usize) {
        let start = nodes[..clicked_idx]
            .iter()
            .rposition(|n| n.item_type == MenuItemType::Separator)
            .map(|i| i + 1)
            .unwrap_or(0);

        let end = nodes[clicked_idx + 1..]
            .iter()
            .position(|n| n.item_type == MenuItemType::Separator)
            .map(|i| clicked_idx + 1 + i)
            .unwrap_or(nodes.len());

        for (idx, item) in nodes[start..end].iter_mut().enumerate() {
            let abs_idx = start + idx;
            if matches!(item.item_type, MenuItemType::Radio(_)) {
                item.item_type = MenuItemType::Radio(abs_idx == clicked_idx);
            }
        }
    }

    fn apply_live_radio_states(
        nodes: &mut [MenuNode],
        clicked_idx: usize,
        live_states: &[(String, bool)],
    ) {
        let start = nodes[..clicked_idx]
            .iter()
            .rposition(|n| n.item_type == MenuItemType::Separator)
            .map(|i| i + 1)
            .unwrap_or(0);

        let end = nodes[clicked_idx + 1..]
            .iter()
            .position(|n| n.item_type == MenuItemType::Separator)
            .map(|i| clicked_idx + 1 + i)
            .unwrap_or(nodes.len());

        for item in &mut nodes[start..end] {
            if matches!(item.item_type, MenuItemType::Radio(_)) {
                let item_norm = item.clean_label().to_lowercase();
                if let Some((_, is_sel)) = live_states
                    .iter()
                    .find(|(lbl, _)| clean_label(lbl).to_lowercase() == item_norm)
                {
                    item.item_type = MenuItemType::Radio(*is_sel);
                }
            }
        }
    }

    pub fn populate_origin_paths(&mut self) {
        let mut current = Vec::new();
        for child in &mut self.children {
            Self::populate_paths_recursive(child, &mut current);
        }
    }

    fn populate_paths_recursive(node: &mut MenuNode, current: &mut Vec<String>) {
        let clean = node.clean_label();
        // Skip synthetic UI nodes so snapshot origin paths stay valid for real live items.
        if !clean.is_empty()
            && !matches!(node.item_type,
                MenuItemType::SearchBar | MenuItemType::WindowList { .. }) {
            current.push(clean);
            node.origin_path = Some(current.clone());
            for child in &mut node.children {
                Self::populate_paths_recursive(child, current);
            }
            current.pop();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtspiSearchConfig {
    pub target_roles: Vec<String>,
    pub name_contains: Option<String>,
    pub traverse_document: bool,
    pub hoist_single_button: bool,
    pub max_depth: usize,
    pub max_nodes_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case")]
pub enum MenuTarget {
    Canonical {
        bus_name: String,
        object_path: String,
    },
    Gtk {
        bus_name: String,
        menubar_path: Option<String>,
        app_menu_path: Option<String>,
        app_path: Option<String>,
        win_path: Option<String>,
    },
    Atspi {
        pid: u32,
        app_bus_name: String,
        menubar_path: String,
        hoist_single_button: bool,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActiveWindowInfo {
    pub pid: u32,
    pub xid: u32,
    pub title: String,
    pub app_id: String,
    pub gtk_bus_name: String,
    pub gtk_menubar_path: String,
    pub gtk_app_menu_path: String,
    pub gtk_app_path: String,
    pub gtk_win_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i32, label: &str, item_type: MenuItemType, children: Vec<MenuNode>) -> MenuNode {
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
    fn clean_label_and_wildcard_match_cover_all_pattern_shapes() {
        assert_eq!(clean_label("  _File & _Open  "), "File & Open");
        assert!(wildcard_match("save*", "_Save As…"));
        assert!(wildcard_match("*window*", "Move to New Window"));
        assert!(wildcard_match("*preferences", "Open Preferences"));
        assert!(wildcard_match("a*b*c", "alpha-beta-doc"));
        assert!(!wildcard_match("a*b*c", "alpha-beta-charlie"));
        assert!(!wildcard_match("", "anything"));
        assert!(wildcard_match("*", "anything"));
        assert!(!wildcard_match("copy", "cut"));
    }

    #[test]
    fn toggle_check_and_separator_bounded_radio_groups_by_id_and_path() {
        let mut root = item(
            0,
            "Root",
            MenuItemType::Standard,
            vec![item(
                1,
                "_View",
                MenuItemType::Submenu,
                vec![
                    item(10, "Show _Sidebar", MenuItemType::Check(false), vec![]),
                    item(11, "", MenuItemType::Separator, vec![]),
                    item(12, "Sort _Name", MenuItemType::Radio(true), vec![]),
                    item(13, "Sort _Date", MenuItemType::Radio(false), vec![]),
                    item(14, "", MenuItemType::Separator, vec![]),
                    item(15, "Theme _Dark", MenuItemType::Radio(true), vec![]),
                ],
            )],
        );

        assert!(root.toggle_check_or_radio_by_id(10));
        assert_eq!(
            root.find_by_id(10).unwrap().item_type,
            MenuItemType::Check(true)
        );

        // Selecting radio 13 must clear sibling 12 without affecting radio 15 past the separator.
        assert!(root.toggle_check_or_radio_by_id(13));
        assert_eq!(
            root.find_by_id(12).unwrap().item_type,
            MenuItemType::Radio(false)
        );
        assert_eq!(
            root.find_by_id(13).unwrap().item_type,
            MenuItemType::Radio(true)
        );
        assert_eq!(
            root.find_by_id(15).unwrap().item_type,
            MenuItemType::Radio(true)
        );

        let path = vec!["View".to_string(), "Sort Name".to_string()];
        let live = vec![
            ("Sort _Name".to_string(), true),
            ("Sort _Date".to_string(), false),
        ];
        assert!(MenuNode::toggle_check_or_radio_by_path(
            &mut root.children,
            &path,
            Some(&live),
        ));
        assert_eq!(
            root.find_by_id(12).unwrap().item_type,
            MenuItemType::Radio(true)
        );
        assert_eq!(
            root.find_by_id(13).unwrap().item_type,
            MenuItemType::Radio(false)
        );
    }

    #[test]
    fn attach_submenu_and_populate_origin_paths_skip_synthetic_nodes() {
        let mut root = item(
            0,
            "Root",
            MenuItemType::Standard,
            vec![
                item(1, "_File", MenuItemType::Submenu, vec![]),
                item(2, "Search", MenuItemType::SearchBar, vec![]),
                item(
                    3,
                    "Windows",
                    MenuItemType::WindowList {
                        min_windows_to_show: 2,
                        max_title_length: 40,
                    },
                    vec![],
                ),
            ],
        );

        assert!(root.attach_submenu_children(
            1,
            vec![item(10, "_New Tab", MenuItemType::Standard, vec![])]
        ));
        assert!(!root.attach_submenu_children(999, vec![]));

        root.populate_origin_paths();
        assert_eq!(
            root.find_by_id(10).unwrap().origin_path,
            Some(vec!["File".to_string(), "New Tab".to_string()])
        );
        assert_eq!(root.find_by_id(2).unwrap().origin_path, None);
        assert_eq!(root.find_by_id(3).unwrap().origin_path, None);
    }
}