use super::ActionState;
use super::proxy::Gtk4BridgeProxy;
use genubar_core::errors::MenuError;
use std::collections::HashMap;
use zbus::{Connection, OwnedVariant, Variant};

/// Converts a structured D-Bus `(enabled, kind, value)` tuple from `org.genubar.Gtk4Bridge`
/// into a normalized [`ActionState`].
pub(crate) fn parse_hook_action_state(enabled: bool, kind: &str, value: &str) -> ActionState {
    let trimmed = value.trim();
    let (state, printed_state) = match kind {
        "bool" => {
            let b = trimmed == "true";
            (Some(OwnedVariant::from(b)), Some(trimmed.to_string()))
        }
        "str" => (
            Variant::from(value).try_to_owned().ok(),
            Some(value.to_string()),
        ),
        "other" if !trimmed.is_empty() => (None, Some(trimmed.to_string())),
        _ => (None, None),
    };

    ActionState {
        enabled,
        state,
        printed_state,
    }
}

pub async fn query_gtk4_zoom_percent(
    connection: &Connection,
    bus_name: &str,
    menubar_path: Option<&str>,
) -> i32 {
    if menubar_path == Some(genubar_protocol::GTK4_MENU_PATH)
        && let Ok(bridge) = Gtk4BridgeProxy::builder(connection)
            .destination(bus_name)
            .build()
            .await
            && let Ok(pct) = bridge.get_zoom_percent().await {
                return pct;
            }
    100
}

/// Queries live structured `GAction` states (`a(sbss)`) from `org.genubar.Gtk4Bridge`.
pub async fn fetch_hook_action_states(
    connection: &Connection,
    bus_name: &str,
    menubar_path: Option<&str>,
) -> HashMap<String, ActionState> {
    let mut out = HashMap::new();
    if menubar_path != Some(genubar_protocol::GTK4_MENU_PATH) {
        return out;
    }
    let Ok(bridge) = Gtk4BridgeProxy::builder(connection)
        .destination(bus_name)
        .build()
        .await
    else {
        return out;
    };
    let Ok(entries) = bridge.get_action_states().await else {
        return out;
    };

    for (full_key, enabled, kind, val_str) in entries {
        out.insert(full_key, parse_hook_action_state(enabled, &kind, &val_str));
    }
    out
}

pub async fn try_activate_bridge_action(
    connection: &Connection,
    bus_name: &str,
    full_action: &str,
    params: Vec<OwnedVariant>,
) -> bool {
    if let Ok(bridge) = Gtk4BridgeProxy::builder(connection)
        .destination(bus_name)
        .build()
        .await
    {
        return bridge.activate(full_action, params).await.is_ok();
    }
    false
}

pub async fn dump_gtk4_introspection(
    connection: &Connection,
    bus_name: &str,
) -> Result<String, MenuError> {
    let bridge = Gtk4BridgeProxy::builder(connection)
        .destination(bus_name)
        .build()
        .await
        .map_err(|e| MenuError::Transport(e.to_string()))?;

    bridge
        .dump_introspection()
        .await
        .map_err(|e| MenuError::Transport(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant;

    #[test]
    fn parses_structured_hook_states_without_tsv_ambiguity() {
        let bool_st = parse_hook_action_state(true, "bool", "true");
        assert!(bool_st.enabled);
        assert!(matches!(bool_st.state.as_deref(), Some(Variant::Bool(true))));

        let str_st = parse_hook_action_state(true, "str", "dark");
        assert_eq!(str_st.printed_state.as_deref(), Some("dark"));
        assert!(matches!(str_st.state.as_deref(), Some(Variant::Str(s)) if s.as_str() == "dark"));

        let tuple_st = parse_hook_action_state(false, "other", "('name', false)");
        assert!(!tuple_st.enabled);
        assert!(tuple_st.state.is_none());
        assert_eq!(tuple_st.printed_state.as_deref(), Some("('name', false)"));
    }
}
