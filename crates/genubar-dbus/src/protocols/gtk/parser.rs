use crate::models::gtk::{ActionState, CachedGtkAction};
use crate::protocols::gtk::custom_widgets::GtkCustomWidgetExpander;
use crate::utils::shortcuts::parse_gtk_accel;
use crate::utils::variant::{get_str_prop, unwrap_val};
use genubar_core::{MenuItemType, MenuNode, clean_label};
use std::collections::{HashMap, HashSet};
use zbus::{OwnedVariant, Variant};

type MenusMap = HashMap<(u32, u32), Vec<HashMap<String, OwnedVariant>>>;

struct GtkParseContext<'a> {
    menus_map: &'a MenusMap,
    action_states: &'a HashMap<String, ActionState>,
    zoom_pct: i32,
    next_id: &'a mut i32,
    cache: &'a mut HashMap<i32, CachedGtkAction>,
    visited: &'a mut HashSet<(u32, u32)>,
    seen_custom: HashSet<String>,
}

pub struct GtkMenuParser;

impl GtkMenuParser {
    pub fn extract_link_tuple(
        props: &HashMap<String, OwnedVariant>,
        key: &str,
    ) -> Option<(u32, u32)> {
        let raw: &Variant = props.get(key)?;
        let Variant::Structure(s) = unwrap_val(raw) else {
            return None;
        };
        let fields = s.fields();
        if fields.len() != 2 {
            return None;
        }
        let group = match unwrap_val(&fields[0]) {
            Variant::U32(n) => *n,
            _ => return None,
        };
        let menu = match unwrap_val(&fields[1]) {
            Variant::U32(n) => *n,
            _ => return None,
        };
        Some((group, menu))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_nodes_from_group(
        group_key: (u32, u32),
        menus_map: &MenusMap,
        action_states: &HashMap<String, ActionState>,
        zoom_pct: i32,
        next_id: &mut i32,
        cache: &mut HashMap<i32, CachedGtkAction>,
        visited: &mut HashSet<(u32, u32)>,
    ) -> Vec<MenuNode> {
        let mut ctx = GtkParseContext {
            menus_map,
            action_states,
            zoom_pct,
            next_id,
            cache,
            visited,
            seen_custom: HashSet::new(),
        };
        Self::build_nodes_inner(group_key, &mut ctx)
    }

    fn build_nodes_inner(group_key: (u32, u32), ctx: &mut GtkParseContext<'_>) -> Vec<MenuNode> {
        let mut result = Vec::new();
        if !ctx.visited.insert(group_key) {
            return result;
        }

        let Some(items) = ctx.menus_map.get(&group_key) else {
            ctx.visited.remove(&group_key);
            return result;
        };

        for item in items {
            if let Some(section_target) = Self::extract_link_tuple(item, ":section") {
                Self::append_section_entry(item, section_target, ctx, &mut result);
                continue;
            }
            Self::append_item_entry(item, ctx, &mut result);
        }

        ctx.visited.remove(&group_key);
        result
    }

    fn append_section_entry(
        item: &HashMap<String, OwnedVariant>,
        section_target: (u32, u32),
        ctx: &mut GtkParseContext<'_>,
        result: &mut Vec<MenuNode>,
    ) {
        let sec_label = get_str_prop(item, "label")
            .map(|s| clean_label(&s))
            .unwrap_or_default();

        let section_nodes = Self::build_nodes_inner(section_target, ctx);
        if section_nodes.is_empty() {
            return;
        }

        // Flatten inline "Icon Size" / "Zoom" / "Rotate" button sections directly into parent column.
        let sec_low = sec_label.to_lowercase();
        let should_wrap_as_submenu = !sec_label.is_empty()
            && !sec_low.contains("icon size")
            && !sec_low.contains("zoom")
            && !sec_low.contains("rotate");

        if should_wrap_as_submenu {
            let sub_id = *ctx.next_id;
            *ctx.next_id += 1;
            result.push(MenuNode {
                id: sub_id,
                label: sec_label,
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                origin_path: None,
                children: section_nodes,
                icon: None,
            });
        } else {
            Self::append_section_separator_if_needed(result, ctx.next_id);
            result.extend(section_nodes);
        }
    }

    fn append_item_entry(
        item: &HashMap<String, OwnedVariant>,
        ctx: &mut GtkParseContext<'_>,
        result: &mut Vec<MenuNode>,
    ) {
        let item_id = *ctx.next_id;
        *ctx.next_id += 1;

        let label = get_str_prop(item, "label").unwrap_or_default();
        let action = get_str_prop(item, "action");
        let hidden_when = get_str_prop(item, "hidden-when");
        let accel = get_str_prop(item, "accel").and_then(|a| parse_gtk_accel(&a));
        let target_param = item.get("target").and_then(|v| v.try_clone().ok());

        if label.trim().is_empty() {
            Self::append_custom_widget_entry(item, ctx, result);
            return;
        }

        if let Some(submenu_target) = Self::extract_link_tuple(item, ":submenu") {
            let children = Self::build_nodes_inner(submenu_target, ctx);
            result.push(MenuNode {
                id: item_id,
                label,
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: accel,
                action,
                origin_path: None,
                children,
                icon: None,
            });
            return;
        }

        let (enabled, item_type, action_known) = Self::resolve_action_state(
            item_id,
            action.as_deref(),
            target_param,
            ctx.action_states,
            ctx.cache,
        );

        if let Some(ref hw) = hidden_when
            && ((hw == "action-disabled" && (!enabled || !action_known))
                || (hw == "action-missing" && !action_known))
            {
                return;
            }

        result.push(MenuNode {
            id: item_id,
            label,
            enabled,
            visible: true,
            item_type,
            shortcut: accel,
            action,
            origin_path: None,
            children: Vec::new(),
            icon: None,
        });
    }

    fn append_custom_widget_entry(
        item: &HashMap<String, OwnedVariant>,
        ctx: &mut GtkParseContext<'_>,
        result: &mut Vec<MenuNode>,
    ) {
        let Some(custom_id) = get_str_prop(item, "custom") else {
            return;
        };
        let cid_low = custom_id.to_lowercase();
        let dedup_key = if cid_low.contains("zoom") {
            String::from("zoom")
        } else if cid_low.contains("theme") || cid_low.contains("style") {
            String::from("theme")
        } else {
            cid_low
        };
        if !ctx.seen_custom.insert(dedup_key) {
            return;
        }
        if let Some(expanded_nodes) = GtkCustomWidgetExpander::expand(
            &custom_id,
            ctx.zoom_pct,
            ctx.action_states,
            ctx.next_id,
            ctx.cache,
        ) {
            Self::append_section_separator_if_needed(result, ctx.next_id);
            result.extend(expanded_nodes);
        }
    }

    fn append_section_separator_if_needed(result: &mut Vec<MenuNode>, next_id: &mut i32) {
        if !result.is_empty()
            && !matches!(
                result.last().map(|n| &n.item_type),
                Some(MenuItemType::Separator)
            )
        {
            let sep_id = *next_id;
            *next_id += 1;
            result.push(MenuNode {
                id: sep_id,
                label: String::new(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Separator,
                shortcut: None,
                action: None,
                origin_path: None,
                children: Vec::new(),
                icon: None,
            });
        }
    }

    fn resolve_action_state(
        item_id: i32,
        action_opt: Option<&str>,
        target_param: Option<OwnedVariant>,
        action_states: &HashMap<String, ActionState>,
        cache: &mut HashMap<i32, CachedGtkAction>,
    ) -> (bool, MenuItemType, bool) {
        let Some(act_name) = action_opt else {
            return (false, MenuItemType::Standard, false);
        };

        cache.insert(
            item_id,
            CachedGtkAction {
                full_action: act_name.to_string(),
                target_param: target_param.clone(),
            },
        );

        let Some(state_info) = action_states.get(act_name) else {
            return (true, MenuItemType::Standard, false);
        };

        let kind = match (&state_info.state, &state_info.printed_state, &target_param) {
            (Some(st), _, None) => match unwrap_val(st) {
                Variant::Bool(b) => MenuItemType::Check(*b),
                _ => MenuItemType::Standard,
            },
            (Some(st), _, Some(targ)) => MenuItemType::Radio(unwrap_val(st) == unwrap_val(targ)),
            (None, Some(printed), Some(targ)) => {
                let is_selected = Self::variant_matches_printed(unwrap_val(targ), printed);
                MenuItemType::Radio(is_selected)
            }
            _ => MenuItemType::Standard,
        };

        let is_dynamic_edit_action = act_name.ends_with(".undo")
            || act_name.ends_with(".redo")
            || act_name.ends_with(".paste")
            || act_name.ends_with(".cut")
            || act_name.ends_with(".copy");

        let enabled = if is_dynamic_edit_action {
            true
        } else {
            state_info.enabled
        };

        (enabled, kind, true)
    }

    // Matches non-primitive zbus Variants (such as `('name', false)`) against `g_variant_print` output.
    fn variant_matches_printed(targ: &Variant<'_>, printed: &str) -> bool {
        match targ {
            Variant::Str(s) => printed.trim_matches('\'') == s.as_str(),
            Variant::Structure(st) => {
                let fields = st.fields();
                if fields.len() == 2
                    && let (Variant::Str(s), Variant::Bool(b)) =
                        (unwrap_val(&fields[0]), unwrap_val(&fields[1]))
                    {
                        let expected = format!("('{s}', {b})");
                        return printed == expected;
                    }
                false
            }
            _ => false,
        }
    }
}
