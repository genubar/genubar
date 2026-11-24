use super::ActionState;
use super::hook_bridge::try_activate_bridge_action;
use super::proxy::GtkActionsProxy;
use genubar_core::ActiveWindowInfo;
use genubar_core::errors::MenuError;
use std::collections::HashMap;
use zbus::{Connection, OwnedVariant};

pub async fn fetch_action_states(
    connection: &Connection,
    bus_name: &str,
    prefix: &str,
    object_path: Option<&str>,
) -> HashMap<String, ActionState> {
    let mut out = HashMap::new();
    let Some(path) = object_path.filter(|p| !p.is_empty()) else {
        return out;
    };

    let Ok(proxy) = GtkActionsProxy::builder(connection)
        .destination(bus_name)
        .path(path)
        .build()
        .await
    else {
        return out;
    };

    if let Ok(descriptions) = proxy.describe_all().await {
        for (name, (enabled, _sig, mut state_vec)) in descriptions {
            let full_key = format!("{prefix}.{name}");
            let state = state_vec.pop();
            out.insert(
                full_key,
                ActionState {
                    enabled,
                    state,
                    printed_state: None,
                },
            );
        }
    }
    out
}

pub async fn activate_org_gtk_action(
    connection: &Connection,
    bus_name: &str,
    object_path: &str,
    short_action: &str,
    params: Vec<OwnedVariant>,
) -> Result<(), MenuError> {
    let proxy = GtkActionsProxy::builder(connection)
        .destination(bus_name)
        .path(object_path)
        .build()
        .await
        .map_err(|e| MenuError::Transport(e.to_string()))?;

    proxy
        .activate(short_action, params, HashMap::new())
        .await
        .map_err(|e| MenuError::Transport(e.to_string()))
}

// Dispatches `gtk:` profile actions both via `org.genubar.Gtk4Bridge` (when hooked)
// and via standard `org.gtk.Actions` on `info.gtk_win_path` / `info.gtk_app_path`.
pub async fn activate_custom_gtk_action(
    connection: &Connection,
    info: &ActiveWindowInfo,
    hooked_bus_name: Option<&str>,
    action_spec: &str,
) -> bool {
    let bus_candidates = [hooked_bus_name, Some(info.gtk_bus_name.as_str())];
    for bus_opt in bus_candidates.into_iter().flatten() {
        if !bus_opt.is_empty()
            && try_activate_bridge_action(connection, bus_opt, action_spec, Vec::new()).await
        {
            return true;
        }
    }

    if info.gtk_bus_name.is_empty() {
        return false;
    }

    for candidate in action_spec
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let clean_cand = candidate.split('(').next().unwrap_or(candidate).trim();
        let (prefix, short_name) = clean_cand.split_once('.').unwrap_or(("win", clean_cand));

        let path_opt = match prefix {
            "app" => Some(info.gtk_app_path.as_str()),
            "win" => Some(info.gtk_win_path.as_str()),
            _ => Some(info.gtk_win_path.as_str()),
        }
        .filter(|p| !p.is_empty());

        if let Some(obj_path) = path_opt
            && activate_org_gtk_action(
                connection,
                info.gtk_bus_name.as_str(),
                obj_path,
                short_name,
                Vec::new(),
            )
            .await
            .is_ok()
        {
            return true;
        }
    }

    false
}
