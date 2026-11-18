use super::menu_opener::trigger_node_action;
use crate::models::atspi::AtspiActionRef;
use crate::protocols::atspi::cache::AtspiSubmenuCache;
use crate::protocols::atspi::parser::AtspiMenuParser;
use crate::protocols::atspi::proxy::{AtspiAccessibleProxy, AtspiActionProxy};
use genubar_core::{MenuItemType, MenuNode, clean_label};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use zbus::Connection;

/// Shared traversal context during recursive AT-SPI2 subtree construction.
pub struct AtspiTraversalContext<'a> {
    pub submenu_cache: &'a AtspiSubmenuCache,
    pub next_uid: &'a AtomicI32,
    pub app_comm: &'a str,
    pub menubar_root: &'a str,
    pub a11y: &'a Connection,
    pub bus_name: &'a str,
    pub expand_nested_menus: bool,
    pub current_path: Vec<String>,
    pub action_cache: HashMap<i32, AtspiActionRef>,
    pub paths_map: HashMap<i32, Vec<String>>,
}

pub async fn find_direct_child_info(
    a11y: &Connection,
    bus_name: &str,
    parent_path: &str,
    target_label: &str,
) -> Option<(String, bool, String, Vec<u32>)> {
    let mut queue = VecDeque::new();
    queue.push_back((parent_path.to_string(), 0usize));

    while let Some((curr_path, depth)) = queue.pop_front() {
        if depth > 2 {
            continue;
        }
        let Ok(proxy) = AtspiAccessibleProxy::builder(a11y)
            .destination(bus_name)
            .path(curr_path.as_str())
            .build()
            .await
        else {
            continue;
        };

        if depth > 0
            && let Ok(name) = proxy.name().await
                && clean_label(&name) == clean_label(target_label) {
                    let role = proxy.get_role_name().await.unwrap_or_default();
                    let state = proxy.get_state().await.unwrap_or_default();
                    let showing = AtspiMenuParser::is_showing(&state);
                    return Some((curr_path, showing, role, state));
                }

        if let Ok(children) = proxy.get_children().await {
            for (c_bus, c_path) in children {
                if c_bus == bus_name {
                    queue.push_back((c_path.to_string(), depth + 1));
                }
            }
        }
    }
    None
}

pub async fn build_subtree(
    ctx: &mut AtspiTraversalContext<'_>,
    object_path: &str,
    depth: usize,
    branch_budget: &mut usize,
) -> Option<MenuNode> {
    if depth > 7 || *branch_budget == 0 {
        return None;
    }
    *branch_budget -= 1;

    let proxy = AtspiAccessibleProxy::builder(ctx.a11y)
        .destination(ctx.bus_name)
        .path(object_path)
        .build()
        .await
        .ok()?;

    let role = proxy.get_role_name().await.unwrap_or_default();
    if role == "separator" {
        let id = ctx.next_uid.fetch_add(1, Ordering::Relaxed);
        return Some(MenuNode {
            id,
            label: String::new(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Separator,
            shortcut: None,
            action: None,
            origin_path: None,
            children: vec![],
            icon: None,
        });
    }

    let label = proxy.name().await.unwrap_or_default();
    let clean = clean_label(&label);
    if clean.is_empty() && !AtspiMenuParser::is_container_role(&role) {
        return None;
    }

    let state_bits = proxy.get_state().await.unwrap_or_default();
    let (enabled, is_checked) = AtspiMenuParser::parse_state_bits(&state_bits);

    let shortcut = if depth >= 2 {
        read_node_shortcut(ctx.a11y, ctx.bus_name, object_path).await
    } else {
        None
    };

    let pushed_segment = if depth >= 1 && !clean.is_empty() {
        ctx.current_path.push(clean.clone());
        true
    } else {
        false
    };

    let mut raw_children = proxy.get_children().await.unwrap_or_default();
    if ctx.expand_nested_menus
        && depth == 2
        && role == "menu"
        && enabled
        && raw_children.is_empty()
        && !clean.is_empty()
    {
        let cache_key = format!("{}::{}", ctx.app_comm, ctx.current_path.join("::"));
        if !ctx.submenu_cache.has_non_empty(&cache_key).await {
            trigger_node_action(
                ctx.a11y,
                ctx.bus_name,
                object_path,
                &format!("Auto-Expand depth=2 submenu '{clean}'"),
            )
            .await;
            sleep(Duration::from_millis(65)).await;
            raw_children = proxy.get_children().await.unwrap_or_default();
        }
    }

    let mut children = Vec::new();
    for (c_bus, c_path) in raw_children {
        if c_bus == ctx.bus_name {
            let mut child_budget = if depth == 0 { 90usize } else { *branch_budget };
            if let Some(child_node) =
                Box::pin(build_subtree(ctx, c_path.as_str(), depth + 1, &mut child_budget)).await
            {
                if depth > 0 {
                    *branch_budget = child_budget;
                }
                if child_node.label.trim().is_empty() && !child_node.children.is_empty() {
                    children.extend(child_node.children);
                } else if !child_node.label.trim().is_empty()
                    || child_node.item_type == MenuItemType::Separator
                {
                    children.push(child_node);
                }
            }
        }
    }

    let id = if depth == 0 {
        0
    } else {
        ctx.next_uid.fetch_add(1, Ordering::Relaxed)
    };

    if pushed_segment {
        ctx.paths_map.insert(id, ctx.current_path.clone());
        let cache_key = format!("{}::{}", ctx.app_comm, ctx.current_path.join("::"));

        if !children.is_empty() {
            ctx.submenu_cache
                .store_submenu(ctx.app_comm, &ctx.current_path, children.clone())
                .await;
        } else if role == "menu"
            && let Some(restored) = ctx
                .submenu_cache
                .restore_and_rebind(
                    ctx.app_comm,
                    &cache_key,
                    ctx.next_uid,
                    ctx.bus_name,
                    ctx.menubar_root,
                    &mut ctx.current_path,
                    &mut ctx.action_cache,
                    &mut ctx.paths_map,
                )
                .await
            {
                children = restored;
            }
    }

    ctx.action_cache.insert(
        id,
        AtspiActionRef::Direct {
            bus_name: ctx.bus_name.to_string(),
            object_path: object_path.to_string(),
        },
    );

    if pushed_segment {
        ctx.current_path.pop();
    }

    let item_type = AtspiMenuParser::resolve_item_type(&role, !children.is_empty(), is_checked);

    Some(MenuNode {
        id,
        label,
        enabled,
        visible: true,
        item_type,
        shortcut,
        action: None,
        origin_path: None,
        children,
        icon: None,
    })
}

async fn read_node_shortcut(
    a11y: &Connection,
    bus_name: &str,
    object_path: &str,
) -> Option<Vec<String>> {
    let act_proxy = AtspiActionProxy::builder(a11y)
        .destination(bus_name)
        .path(object_path)
        .build()
        .await
        .ok()?;
    let kb = act_proxy.get_key_binding(0).await.ok()?;
    AtspiMenuParser::parse_shortcut(&kb)
}
