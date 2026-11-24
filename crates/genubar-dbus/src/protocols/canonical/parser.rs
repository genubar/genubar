use crate::utils::variant::{get_bool_prop, get_i32_prop, get_str_prop, unwrap_val};
use genubar_core::errors::MenuError;
use genubar_core::{MenuItemType, MenuNode, clean_label};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use zbus::{OwnedVariant, Variant};

pub struct CanonicalMenuParser;

impl CanonicalMenuParser {
    pub fn parse_node(
        id: i32,
        depth: usize,
        props: &HashMap<String, OwnedVariant>,
        raw_children: &[OwnedVariant],
    ) -> Result<MenuNode, MenuError> {
        let label = get_str_prop(props, "label").unwrap_or_default();
        let enabled = get_bool_prop(props, "enabled", true);
        let visible = get_bool_prop(props, "visible", true);

        let mut item_type = Self::extract_item_type(props, !raw_children.is_empty());
        if depth == 1 && item_type != MenuItemType::Separator {
            item_type = MenuItemType::Submenu;
        }

        let shortcut = Self::extract_shortcut(props);
        let mut children = Vec::with_capacity(raw_children.len());

        for child_val in raw_children {
            if let Some(parsed_child) = Self::parse_child_variant(child_val, depth + 1)? {
                children.push(parsed_child);
            }
        }

        if item_type == MenuItemType::Submenu
            && !children.is_empty()
            && !Self::slice_has_meaningful_items(&children)
        {
            children.clear();
        }

        Ok(MenuNode {
            id,
            label,
            enabled,
            visible,
            item_type,
            shortcut,
            action: None,
            origin_path: None,
            children,
            icon: None,
        })
    }

    fn parse_child_variant(
        child_val: &OwnedVariant,
        next_depth: usize,
    ) -> Result<Option<MenuNode>, MenuError> {
        let Variant::Structure(structure) = unwrap_val(child_val) else {
            return Ok(None);
        };
        let fields = structure.fields();
        if fields.len() != 3 {
            return Ok(None);
        }

        let child_id = match unwrap_val(&fields[0]) {
            Variant::I32(n) => *n,
            other => {
                return Err(MenuError::ParseError(format!(
                    "Expected i32 child ID, got {other:?}"
                )));
            }
        };

        let child_props: HashMap<String, OwnedVariant> = HashMap::try_from(
            fields[1]
                .try_to_owned()
                .map_err(|e| MenuError::ParseError(format!("Failed to clone props: {e}")))?,
        )
        .map_err(|e| MenuError::ParseError(format!("Invalid props map: {e}")))?;

        let child_sub_array: Vec<OwnedVariant> =
            Vec::try_from(fields[2].try_to_owned().map_err(|e| {
                MenuError::ParseError(format!("Failed to clone children array: {e}"))
            })?)
            .map_err(|e| MenuError::ParseError(format!("Invalid children vec: {e}")))?;

        let node = Self::parse_node(child_id, next_depth, &child_props, &child_sub_array)?;
        Ok(Some(node))
    }

    fn extract_item_type(
        props: &HashMap<String, OwnedVariant>,
        has_children: bool,
    ) -> MenuItemType {
        if get_str_prop(props, "type").as_deref() == Some("separator") {
            return MenuItemType::Separator;
        }
        if get_str_prop(props, "children-display").as_deref() == Some("submenu") || has_children {
            return MenuItemType::Submenu;
        }
        if let Some(toggle_type) = get_str_prop(props, "toggle-type") {
            let state = get_i32_prop(props, "toggle-state", 0) == 1;
            return match toggle_type.as_str() {
                "checkmark" => MenuItemType::Check(state),
                "radio" => MenuItemType::Radio(state),
                _ => MenuItemType::Standard,
            };
        }
        MenuItemType::Standard
    }

    fn extract_shortcut(props: &HashMap<String, OwnedVariant>) -> Option<Vec<String>> {
        let val = props.get("shortcut")?;
        let outer: Vec<Vec<String>> = Vec::try_from(val.try_clone().ok()?).ok()?;
        outer.into_iter().next()
    }

    pub fn has_meaningful_children(node: &MenuNode) -> bool {
        Self::slice_has_meaningful_items(&node.children)
    }

    fn slice_has_meaningful_items(children: &[MenuNode]) -> bool {
        children.iter().any(|c| {
            c.visible && c.item_type != MenuItemType::Separator && !clean_label(&c.label).is_empty()
        })
    }

    pub fn assign_uids_and_index_paths(
        node: &mut MenuNode,
        is_root: bool,
        next_uid: &AtomicI32,
        current_path: &mut Vec<String>,
        map: &mut HashMap<i32, Vec<String>>,
    ) {
        if !is_root {
            let clean = clean_label(&node.label);
            let uid = next_uid.fetch_add(1, Ordering::Relaxed);
            node.id = uid;
            if !clean.is_empty() {
                current_path.push(clean);
                map.insert(uid, current_path.clone());
            } else {
                return;
            }
        } else {
            node.id = 0;
        }

        for child in &mut node.children {
            Self::assign_uids_and_index_paths(child, false, next_uid, current_path, map);
        }

        if !is_root {
            current_path.pop();
        }
    }
}
