use super::menu_opener::open_path_chain;
use crate::protocols::atspi::parser::AtspiMenuParser;
use crate::protocols::atspi::proxy::AtspiAccessibleProxy;
use genubar_core::clean_label;
use std::collections::VecDeque;
use zbus::Connection;

pub async fn read_live_sibling_states(
    a11y: &Connection,
    bus_name: &str,
    menubar_path: &str,
    parent_label_path: &[String],
) -> Option<Vec<(String, bool)>> {
    if parent_label_path.is_empty() {
        return None;
    }
    let parent_obj_path =
        open_path_chain(a11y, bus_name, menubar_path, parent_label_path, false).await?;

    let mut queue = VecDeque::new();
    queue.push_back((parent_obj_path, 0usize));
    let mut states = Vec::new();

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

        if depth > 0 {
            let name = proxy.name().await.unwrap_or_default();
            let clean = clean_label(&name);
            if !clean.is_empty() {
                let state_bits = proxy.get_state().await.unwrap_or_default();
                let (_enabled, is_checked) = AtspiMenuParser::parse_state_bits(&state_bits);
                states.push((clean, is_checked));
            }
        }

        if let Ok(children) = proxy.get_children().await {
            for (c_bus, c_path) in children {
                if c_bus == bus_name {
                    queue.push_back((c_path.to_string(), depth + 1));
                }
            }
        }
    }

    if states.is_empty() {
        None
    } else {
        Some(states)
    }
}
