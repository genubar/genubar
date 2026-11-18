use super::tree_walker::{AtspiTraversalContext, build_subtree, find_direct_child_info};
use crate::models::atspi::AtspiActionRef;
use crate::protocols::atspi::cache::AtspiSubmenuCache;
use crate::protocols::atspi::parser::AtspiMenuParser;
use crate::protocols::atspi::proxy::AtspiActionProxy;
use genubar_core::MenuNode;
use std::collections::HashMap;
use std::sync::atomic::AtomicI32;
use std::time::Duration;
use tokio::time::sleep;
use zbus::Connection;

pub async fn trigger_node_action(
    a11y: &Connection,
    bus_name: &str,
    object_path: &str,
    context_tag: &str,
) -> bool {
    let Ok(act_proxy) = AtspiActionProxy::builder(a11y)
        .destination(bus_name)
        .path(object_path)
        .build()
        .await
    else {
        tracing::debug!(context_tag, object_path, "Failed to build AtspiActionProxy");
        return false;
    };

    if tracing::enabled!(tracing::Level::DEBUG) {
        let count = act_proxy.n_actions().await.unwrap_or(0);
        let act0_name = act_proxy.get_name(0).await.unwrap_or_default();
        tracing::debug!(
            context_tag,
            object_path,
            n_actions = count,
            action0 = %act0_name,
            "Triggering AT-SPI2 node action"
        );
    }

    let res = act_proxy.do_action(0).await;
    tracing::debug!(context_tag, object_path, result = ?res, "AT-SPI2 DoAction(0) completed");
    res.unwrap_or(false)
}

pub async fn open_path_chain(
    a11y: &Connection,
    bus_name: &str,
    menubar_path: &str,
    label_path: &[String],
    force_open_parents: bool,
) -> Option<String> {
    let mut current_obj_path = menubar_path.to_string();

    for (idx, segment) in label_path.iter().enumerate() {
        let is_intermediate_parent = idx + 1 < label_path.len();
        let mut info =
            find_direct_child_info(a11y, bus_name, &current_obj_path, segment).await;

        if info.is_none() {
            trigger_node_action(
                a11y,
                bus_name,
                &current_obj_path,
                &format!("Step {idx} (parent of missing '{segment}')"),
            )
            .await;

            for _attempt in 0..8 {
                sleep(Duration::from_millis(45)).await;
                info = find_direct_child_info(a11y, bus_name, &current_obj_path, segment).await;
                if info.is_some() {
                    break;
                }
            }
        }

        let (child_path, showing, role, st) = info?;
        tracing::debug!(
            step = idx,
            segment = %segment,
            child_path = %child_path,
            role = %role,
            showing,
            state = %AtspiMenuParser::format_state_bits(&st),
            "Located AT-SPI2 chain segment"
        );

        if is_intermediate_parent && force_open_parents {
            trigger_node_action(
                a11y,
                bus_name,
                &child_path,
                &format!("Force-Open Parent Step {idx} ('{segment}')"),
            )
            .await;
            sleep(Duration::from_millis(85)).await;
        }

        current_obj_path = child_path;
    }

    Some(current_obj_path)
}

#[allow(clippy::too_many_arguments)]
pub async fn poll_target_subtree(
    submenu_cache: &AtspiSubmenuCache,
    next_uid: &AtomicI32,
    app_comm: &str,
    menubar_path: &str,
    a11y: &Connection,
    app_bus_name: &str,
    target_obj_path: &str,
    label_path: &[String],
    expand_nested: bool,
    max_polls: usize,
    submenu_id: i32,
) -> (
    Option<MenuNode>,
    HashMap<i32, AtspiActionRef>,
    HashMap<i32, Vec<String>>,
) {
    let mut parsed_node = None;
    let mut final_cache = HashMap::new();
    let mut final_paths = HashMap::new();

    for attempt in 0..max_polls {
        sleep(Duration::from_millis(45)).await;
        let mut ctx = AtspiTraversalContext {
            submenu_cache,
            next_uid,
            app_comm,
            menubar_root: menubar_path,
            a11y,
            bus_name: app_bus_name,
            expand_nested_menus: expand_nested,
            current_path: label_path[..label_path.len().saturating_sub(1)].to_vec(),
            action_cache: HashMap::new(),
            paths_map: HashMap::new(),
        };
        let mut budget = 150usize;

        if let Some(node) =
            build_subtree(&mut ctx, target_obj_path, label_path.len(), &mut budget).await
        {
            let item_count = node.children.len();
            tracing::debug!(
                submenu_id,
                attempt = attempt + 1,
                target_obj_path,
                item_count,
                "Polled AT-SPI2 submenu children"
            );
            parsed_node = Some(node);
            final_cache = ctx.action_cache;
            final_paths = ctx.paths_map;
            if item_count > 0 {
                break;
            }
        }
    }

    (parsed_node, final_cache, final_paths)
}
