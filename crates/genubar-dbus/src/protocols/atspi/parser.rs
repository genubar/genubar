use crate::models::atspi::AtspiActionRef;
use crate::utils::shortcuts::parse_atspi_shortcut;
use genubar_core::{MenuItemType, MenuNode, clean_label};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};

pub struct AtspiMenuParser;

impl AtspiMenuParser {
    pub fn is_heavy_container_role(role: &str) -> bool {
        matches!(
            role,
            "document web"
                | "document frame"
                | "document text"
                | "scroll pane"
                | "tree"
                | "table"
                | "terminal"
        )
    }

    pub fn parse_shortcut(raw: &str) -> Option<Vec<String>> {
        parse_atspi_shortcut(raw)
    }

    pub fn parse_state_bits(state_bits: &[u32]) -> (bool, bool) {
        let low_bits = state_bits.first().copied().unwrap_or(0);
        let enabled = if state_bits.is_empty() {
            true
        } else {
            (low_bits & (1 << 24)) != 0 && (low_bits & (1 << 8)) != 0
        };
        let is_checked = (low_bits & (1 << 4)) != 0;
        (enabled, is_checked)
    }

    pub fn is_showing(state_bits: &[u32]) -> bool {
        let low_bits = state_bits.first().copied().unwrap_or(0);
        (low_bits & (1 << 25)) != 0
    }

    pub fn format_state_bits(state_bits: &[u32]) -> String {
        let low = state_bits.first().copied().unwrap_or(0);
        let flags: Vec<&str> = [
            (1, "ACTIVE"),
            (4, "CHECKED"),
            (8, "ENABLED"),
            (9, "EXPANDABLE"),
            (10, "EXPANDED"),
            (12, "FOCUSED"),
            (23, "SELECTED"),
            (24, "SENSITIVE"),
            (25, "SHOWING"),
            (30, "VISIBLE"),
        ]
        .iter()
        .filter_map(|(bit, name)| ((low & (1 << bit)) != 0).then_some(*name))
        .collect();

        format!("[{}]", flags.join("|"))
    }

    pub fn is_container_role(role: &str) -> bool {
        matches!(
            role,
            "menu bar" | "menu" | "panel" | "pop up button" | "menu button" | "push button"
        )
    }

    pub fn resolve_item_type(role: &str, has_children: bool, is_checked: bool) -> MenuItemType {
        if has_children || matches!(role, "menu" | "menu bar" | "pop up button" | "menu button") {
            MenuItemType::Submenu
        } else if role == "check menu item" {
            MenuItemType::Check(is_checked)
        } else if role == "radio menu item" {
            MenuItemType::Radio(is_checked)
        } else {
            MenuItemType::Standard
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rebind_cached_nodes(
        app_comm: &str,
        submenu_cache: &HashMap<String, Vec<MenuNode>>,
        nodes: &mut [MenuNode],
        next_uid: &AtomicI32,
        bus_name: &str,
        menubar_path: &str,
        current_path: &mut Vec<String>,
        cache: &mut HashMap<i32, AtspiActionRef>,
        paths_map: &mut HashMap<i32, Vec<String>>,
    ) {
        for node in nodes {
            let uid = next_uid.fetch_add(1, Ordering::Relaxed);
            node.id = uid;
            let clean = clean_label(&node.label);

            if node.item_type != MenuItemType::Separator && !clean.is_empty() {
                current_path.push(clean);
                paths_map.insert(uid, current_path.clone());
                cache.insert(
                    uid,
                    AtspiActionRef::CachedReplay {
                        bus_name: bus_name.to_string(),
                        menubar_path: menubar_path.to_string(),
                        label_path: current_path.clone(),
                    },
                );

                if node.item_type == MenuItemType::Submenu {
                    let nested_key = format!("{app_comm}::{}", current_path.join("::"));
                    if let Some(nested_children) = submenu_cache.get(&nested_key)
                        && !nested_children.is_empty()
                    {
                        node.children = nested_children.clone();
                    }
                }

                if !node.children.is_empty() {
                    Self::rebind_cached_nodes(
                        app_comm,
                        submenu_cache,
                        &mut node.children,
                        next_uid,
                        bus_name,
                        menubar_path,
                        current_path,
                        cache,
                        paths_map,
                    );
                }
                current_path.pop();
            }
        }
    }
}
