use crate::fallback::FallbackMenuBuilder;
use crate::models::MenuPayload;
use crate::resolver::WindowMenuResolver;
use crate::state::DaemonCaches;
use crate::tier_acquirer::TierAcquirer;
use genubar_config::ConfigManager;
use genubar_core::{
    ActiveWindowInfo, MenuItemType, MenuNode, MenuProvider, MenuSourceTier, MenuTarget,
};
use genubar_dbus::UnifiedMenuService;
use genubar_pipeline::MenuPipeline;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SubmenuService;

impl SubmenuService {
    /// Fetches a lazy top-level submenu over D-Bus/AT-SPI2 and merges matching profile and `app_menu.toml` rules.
    pub async fn fetch_and_process_submenu(
        service: &UnifiedMenuService,
        caches: &DaemonCaches,
        cfg: &Arc<ConfigManager>,
        info: &ActiveWindowInfo,
        target: &MenuTarget,
        current_menu: &RwLock<Option<MenuPayload>>,
        item_id: i32,
    ) -> Option<MenuNode> {
        let primary = match target {
            MenuTarget::Canonical { .. } => {
                let opts = TierAcquirer::canonical_options(cfg);
                service
                    .canonical()
                    .fetch_submenu_with_options(target, item_id, opts)
                    .await
            }
            _ => service.fetch_submenu(target, item_id).await,
        };

        let fetched = match primary {
            Ok(node) => node,
            Err(primary_err) => {
                let fallback_path = Self::resolve_label_path_from_menu(current_menu, item_id).await;
                tracing::debug!(
                    item_id,
                    error = %primary_err,
                    has_fallback_path = fallback_path.is_some(),
                    "Primary FetchSubmenu failed; trying current_menu label-path fallback"
                );
                let path = fallback_path?;
                match target {
                    MenuTarget::Canonical { .. } => {
                        let opts = TierAcquirer::canonical_options(cfg);
                        service
                            .canonical()
                            .fetch_submenu_by_path_with_options(target, item_id, path, opts)
                            .await
                            .ok()?
                    }
                    MenuTarget::Atspi { .. } => service
                        .atspi()
                        .fetch_submenu_by_path(target, item_id, path)
                        .await
                        .ok()?,
                    MenuTarget::Gtk { .. } => return None,
                }
            }
        };

        // Cache raw Canonical submenu in RAM for instant subsequent focus switches
        if matches!(target, MenuTarget::Canonical { .. }) {
            caches
                .canonical
                .cache_submenu(info.pid, &fetched.clean_label(), fetched.children.clone())
                .await;
        }

        let tier = match target {
            MenuTarget::Gtk { .. } => MenuSourceTier::GtkNative,
            MenuTarget::Canonical { .. } => MenuSourceTier::CanonicalDbus,
            MenuTarget::Atspi { .. } => MenuSourceTier::Atspi,
        };

        let cfg_clone = Arc::clone(cfg);
        let info_clone = info.clone();
        let final_submenu = tokio::task::spawn_blocking(move || {
            Self::process_submenu_blocking(&cfg_clone, &info_clone, tier, fetched)
        })
        .await
        .ok()?;

        if let Some(ref mut payload) = *current_menu.write().await {
            payload
                .root
                .attach_submenu_children(item_id, final_submenu.children.clone());
        }

        Some(final_submenu)
    }

    async fn resolve_label_path_from_menu(
        current_menu: &RwLock<Option<MenuPayload>>,
        item_id: i32,
    ) -> Option<Vec<String>> {
        let guard = current_menu.read().await;
        let payload = guard.as_ref()?;
        let mut curr = Vec::new();
        Self::find_label_path_recursive(&payload.root, true, item_id, &mut curr)
    }

    fn find_label_path_recursive(
        node: &MenuNode,
        is_root: bool,
        target_id: i32,
        curr: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if !is_root {
            let clean = node.clean_label();
            if clean.is_empty() {
                return None;
            }
            curr.push(clean);
            if node.id == target_id {
                return Some(
                    node.origin_path
                        .as_ref()
                        .filter(|p| !p.is_empty())
                        .cloned()
                        .unwrap_or_else(|| curr.clone()),
                );
            }
        }
        for child in &node.children {
            if let Some(found) = Self::find_label_path_recursive(child, false, target_id, curr) {
                return Some(found);
            }
        }
        if !is_root {
            curr.pop();
        }
        None
    }

    fn process_submenu_blocking(
        cfg: &ConfigManager,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        mut submenu_node: MenuNode,
    ) -> MenuNode {
        let app_name = FallbackMenuBuilder::clean_app_name(info);
        let runtime = WindowMenuResolver::runtime_config(cfg);

        for profile in cfg.find_matching_profiles(info) {
            submenu_node = MenuPipeline::apply_profile_to_fetched_submenu(
                submenu_node,
                profile,
                &app_name,
                MenuSourceTier::AppProfile,
            );
        }

        submenu_node = MenuPipeline::apply_app_menu_to_fetched_submenu(
            submenu_node,
            &cfg.app_menu,
            &app_name,
            tier,
        );

        let mut temp_root = MenuNode {
            id: 0,
            label: String::new(),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![submenu_node],
        };
        MenuPipeline::apply_menu_icons_with_config(
            &mut temp_root,
            info,
            &cfg.icons,
            &cfg.icon_packs,
            runtime,
        );
        let strip_refresh = tier == MenuSourceTier::Atspi
            && !cfg.settings.api_tiers.atspi.append_refresh_cache_button;
        MenuPipeline::apply_runtime_filters(&mut temp_root, runtime, strip_refresh);
        temp_root.children.remove(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_label_path_recursive_resolves_origin_paths_and_nested_labels() {
        let root = MenuNode {
            id: 0,
            label: String::from("Root"),
            enabled: true,
            visible: true,
            item_type: MenuItemType::Submenu,
            shortcut: None,
            action: None,
            icon: None,
            origin_path: None,
            children: vec![MenuNode {
                id: 50,
                label: String::from("_Code"),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Submenu,
                shortcut: None,
                action: None,
                icon: None,
                origin_path: None,
                children: vec![MenuNode {
                    id: 51,
                    label: String::from("_Generate"),
                    enabled: true,
                    visible: true,
                    item_type: MenuItemType::Submenu,
                    shortcut: None,
                    action: None,
                    icon: None,
                    origin_path: Some(vec![String::from("Code"), String::from("Generate...")]),
                    children: vec![],
                }],
            }],
        };

        let mut curr = Vec::new();
        assert_eq!(
            SubmenuService::find_label_path_recursive(&root, true, 50, &mut curr),
            Some(vec![String::from("Code")])
        );
        curr.clear();
        assert_eq!(
            SubmenuService::find_label_path_recursive(&root, true, 51, &mut curr),
            Some(vec![String::from("Code"), String::from("Generate...")])
        );
        curr.clear();
        assert_eq!(
            SubmenuService::find_label_path_recursive(&root, true, 999, &mut curr),
            None
        );
    }
}
