use crate::models::gtk::{ActionState, CachedGtkAction};
use crate::utils::variant::unwrap_val;
use genubar_core::{MenuItemType, MenuNode};
use genubar_protocol::{
    ACTION_THEME_DARK, ACTION_THEME_LIGHT, ACTION_THEME_SYSTEM, ACTION_ZOOM_IN, ACTION_ZOOM_OUT,
    ACTION_ZOOM_RESET,
};
use std::collections::HashMap;
use zbus::Variant;

pub struct GtkCustomWidgetExpander;

impl GtkCustomWidgetExpander {
    pub fn expand(
        custom_id: &str,
        zoom_pct: i32,
        action_states: &HashMap<String, ActionState>,
        next_id: &mut i32,
        cache: &mut HashMap<i32, CachedGtkAction>,
    ) -> Option<Vec<MenuNode>> {
        let cid = custom_id.to_lowercase();
        if cid.contains("zoom") {
            return Some(Self::build_zoom_nodes(zoom_pct, next_id, cache));
        }
        if cid.contains("theme") || cid.contains("style") {
            return Some(Self::build_theme_nodes(action_states, next_id, cache));
        }
        if cid.contains("clipboard") {
            return Some(Self::build_clipboard_nodes(next_id));
        }
        if cid == "rotate-left" {
            return Some(Self::build_single_action_node(
                "Rotate Left",
                "win.rotate-ccw",
                Some(vec![
                    "Ctrl".to_string(),
                    "Shift".to_string(),
                    "R".to_string(),
                ]),
                next_id,
                cache,
            ));
        }
        if cid == "rotate-right" {
            return Some(Self::build_single_action_node(
                "Rotate Right",
                "win.rotate-cw",
                Some(vec!["Ctrl".to_string(), "R".to_string()]),
                next_id,
                cache,
            ));
        }
        None
    }

    fn build_single_action_node(
        label: &str,
        full_action: &str,
        shortcut: Option<Vec<String>>,
        next_id: &mut i32,
        cache: &mut HashMap<i32, CachedGtkAction>,
    ) -> Vec<MenuNode> {
        let id = *next_id;
        *next_id += 1;
        cache.insert(
            id,
            CachedGtkAction {
                full_action: full_action.to_string(),
                target_param: None,
            },
        );
        vec![MenuNode {
            id,
            label: label.to_string(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Standard,
            shortcut,
            action: Some(full_action.to_string()),
            icon: None,
            origin_path: None,
            children: vec![],
        }]
    }

    fn build_zoom_nodes(
        zoom_pct: i32,
        next_id: &mut i32,
        cache: &mut HashMap<i32, CachedGtkAction>,
    ) -> Vec<MenuNode> {
        let mut nodes = Vec::new();
        let ind_id = *next_id;
        *next_id += 1;
        nodes.push(MenuNode {
            id: ind_id,
            label: format!("Zoom: {zoom_pct}%"),
            enabled: false,
            visible: true,
            item_type: MenuItemType::Standard,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![],
        });

        for (lbl, bridge_act, sc) in [
            (
                "Zoom In",
                ACTION_ZOOM_IN,
                Some(vec!["Ctrl".to_string(), "+".to_string()]),
            ),
            (
                "Zoom Out",
                ACTION_ZOOM_OUT,
                Some(vec!["Ctrl".to_string(), "-".to_string()]),
            ),
            (
                "Reset Zoom",
                ACTION_ZOOM_RESET,
                Some(vec!["Ctrl".to_string(), "0".to_string()]),
            ),
        ] {
            let id = *next_id;
            *next_id += 1;
            cache.insert(
                id,
                CachedGtkAction {
                    full_action: bridge_act.to_string(),
                    target_param: None,
                },
            );
            nodes.push(MenuNode {
                id,
                label: lbl.to_string(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Standard,
                shortcut: sc,
                action: Some(bridge_act.to_string()),
                icon: None,
                origin_path: None,
                children: vec![],
            });
        }
        nodes
    }

    fn active_theme_bridge_action(action_states: &HashMap<String, ActionState>) -> &'static str {
        for key in [
            "genubar.theme",
            "win.interface-style",
            "settings.style-variant",
            "app.style-scheme",
        ] {
            let Some(st) = action_states.get(key) else {
                continue;
            };
            let val_opt: Option<String> = if let Some(ref printed) = st.printed_state {
                Some(printed.trim_matches('\'').to_lowercase())
            } else if let Some(ref var) = st.state {
                if let Variant::Str(s) = unwrap_val(var) {
                    Some(s.as_str().to_lowercase())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(val) = val_opt {
                if val.contains("light") {
                    return ACTION_THEME_LIGHT;
                }
                if val.contains("dark") {
                    return ACTION_THEME_DARK;
                }
                return ACTION_THEME_SYSTEM;
            }
        }
        ACTION_THEME_SYSTEM
    }

    fn build_theme_nodes(
        action_states: &HashMap<String, ActionState>,
        next_id: &mut i32,
        cache: &mut HashMap<i32, CachedGtkAction>,
    ) -> Vec<MenuNode> {
        let active_bridge_act = Self::active_theme_bridge_action(action_states);
        let mut theme_items = Vec::new();
        for (lbl, bridge_act) in [
            ("System Theme", ACTION_THEME_SYSTEM),
            ("Light Theme", ACTION_THEME_LIGHT),
            ("Dark Theme", ACTION_THEME_DARK),
        ] {
            let id = *next_id;
            *next_id += 1;
            cache.insert(
                id,
                CachedGtkAction {
                    full_action: bridge_act.to_string(),
                    target_param: None,
                },
            );
            theme_items.push(MenuNode {
                id,
                label: lbl.to_string(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Radio(bridge_act == active_bridge_act),
                shortcut: None,
                action: Some(bridge_act.to_string()),
                icon: None,
                origin_path: None,
                children: vec![],
            });
        }

        let sub_id = *next_id;
        *next_id += 1;
        vec![MenuNode {
            id: sub_id,
            label: String::from("Appearance Theme"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: theme_items,
        }]
    }

    fn build_clipboard_nodes(next_id: &mut i32) -> Vec<MenuNode> {
        let mut nodes = Vec::new();
        for (lbl, key_act, sc) in [
            (
                "Cut",
                "key:Ctrl+X",
                vec!["Ctrl".to_string(), "X".to_string()],
            ),
            (
                "Copy",
                "key:Ctrl+C",
                vec!["Ctrl".to_string(), "C".to_string()],
            ),
            (
                "Paste",
                "key:Ctrl+V",
                vec!["Ctrl".to_string(), "V".to_string()],
            ),
        ] {
            let id = *next_id;
            *next_id += 1;
            nodes.push(MenuNode {
                id,
                label: lbl.to_string(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Standard,
                shortcut: Some(sc),
                action: Some(key_act.to_string()),
                icon: None,
                origin_path: None,
                children: vec![],
            });
        }
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_widget_expander_uses_protocol_action_ids_for_zoom_and_theme() {
        let states = HashMap::from([(
            String::from("genubar.theme"),
            ActionState {
                enabled: true,
                state: None,
                printed_state: Some(String::from("'dark'")),
            },
        )]);
        let mut next_id = 10;
        let mut cache = HashMap::new();

        let zoom =
            GtkCustomWidgetExpander::expand("zoom-control", 125, &states, &mut next_id, &mut cache)
                .unwrap();
        assert_eq!(zoom[0].label, "Zoom: 125%");
        let zoom_actions: Vec<&str> = zoom.iter().filter_map(|n| n.action.as_deref()).collect();
        assert_eq!(
            zoom_actions,
            vec![ACTION_ZOOM_IN, ACTION_ZOOM_OUT, ACTION_ZOOM_RESET]
        );
        assert!(cache.values().any(|c| c.full_action == ACTION_ZOOM_IN));

        let theme = GtkCustomWidgetExpander::expand(
            "theme-selector",
            100,
            &states,
            &mut next_id,
            &mut cache,
        )
        .unwrap();
        assert_eq!(theme.len(), 1);
        assert_eq!(theme[0].children.len(), 3);
        let dark_item = theme[0]
            .children
            .iter()
            .find(|n| n.action.as_deref() == Some(ACTION_THEME_DARK))
            .unwrap();
        assert_eq!(dark_item.item_type, MenuItemType::Radio(true));
        let light_item = theme[0]
            .children
            .iter()
            .find(|n| n.action.as_deref() == Some(ACTION_THEME_LIGHT))
            .unwrap();
        assert_eq!(light_item.item_type, MenuItemType::Radio(false));
    }
}
