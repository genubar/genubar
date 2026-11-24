use crate::models::menu::MenuNodeDef;
use crate::models::profile::{AppMatcher, AppProfile, MatchField};
use crate::models::snapshot::MenuSnapshot;
use crate::utils::slug::sanitize_slug;
use crate::utils::toml_io::write_toml_file;
use genubar_core::{ActiveWindowInfo, MenuError, MenuItemType, MenuNode, MenuSourceTier};
use genubar_platform::fs_utils::{
    ensure_dir, read_text_file as read_string, write_text_file as write_string,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SnapshotStore {
    snapshots_dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(snapshots_dir: PathBuf) -> Self {
        let _ = ensure_dir(&snapshots_dir);
        Self { snapshots_dir }
    }

    pub fn save_snapshot(
        &self,
        info: &ActiveWindowInfo,
        tier: MenuSourceTier,
        root: &MenuNode,
    ) -> Result<PathBuf, MenuError> {
        let slug = sanitize_slug(&info.app_id, &info.title);
        let file_path = self.snapshots_dir.join(format!("{slug}.json"));

        let mut annotated_root = root.clone();
        annotated_root.children.retain(|col| col.id != -10);
        annotated_root.populate_origin_paths();

        let snapshot = MenuSnapshot {
            app_id: info.app_id.clone(),
            window_title: info.title.clone(),
            captured_from_tier: tier,
            root: annotated_root,
        };

        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| MenuError::Config(format!("Failed to serialize snapshot: {e}")))?;
        write_string(&file_path, &json)?;
        Ok(file_path)
    }

    pub fn load_snapshot(&self, file_name_or_path: &Path) -> Result<MenuSnapshot, MenuError> {
        let full_path = if file_name_or_path.is_absolute() {
            file_name_or_path.to_path_buf()
        } else {
            self.snapshots_dir.join(file_name_or_path)
        };
        let raw = read_string(&full_path)?;
        serde_json::from_str::<MenuSnapshot>(&raw)
            .map_err(|e| MenuError::Config(format!("Invalid snapshot JSON: {e}")))
    }

    pub fn create_profile_from_snapshot(
        &self,
        snapshot_file: &Path,
        profiles_dir: &Path,
        sync_new_items: bool,
    ) -> Result<PathBuf, MenuError> {
        let snap = self.load_snapshot(snapshot_file)?;
        let slug = sanitize_slug(&snap.app_id, &snap.window_title);
        let match_val = if !snap.app_id.is_empty() {
            snap.app_id.clone()
        } else {
            snap.window_title.clone()
        };

        let mut columns: Vec<MenuNodeDef> = snap
            .root
            .children
            .iter()
            .map(|col| Self::node_to_inherit_def(col, sync_new_items))
            .collect();

        if columns.is_empty() {
            columns.push(MenuNodeDef::InheritOthers);
        }

        let out_path = profiles_dir.join(format!("{slug}.toml"));
        let profile = AppProfile {
            id: slug,
            name: format!("Profile for {match_val}"),
            version: String::from("1.0.0"),
            author: String::from("local"),
            description: format!("Auto-generated profile from snapshot of {match_val}"),
            min_app_version: None,
            enabled: true,
            priority: 0,
            matchers: vec![AppMatcher {
                field: if !snap.app_id.is_empty() {
                    MatchField::AppId
                } else {
                    MatchField::Title
                },
                contains: match_val,
            }],
            sync_new_items,
            include_app_system_column: true,
            fallback_only: false,
            disable_root_sort: false,
            allowed_tiers: None,
            atspi_override: None,
            volatile_submenus: vec![],
            columns,
            python_script: None,
            file_path: out_path.clone(),
        };

        write_toml_file(&out_path, &profile)?;
        Ok(out_path)
    }

    fn node_to_inherit_def(node: &MenuNode, sync_new_items: bool) -> MenuNodeDef {
        if node.item_type == MenuItemType::Separator {
            return MenuNodeDef::Separator {
                visible: node.visible,
                only_tiers: None,
            };
        }

        let clean = node.clean_label();
        let child_defs = if node.children.is_empty() {
            None
        } else {
            Some(
                node.children
                    .iter()
                    .map(|c| Self::node_to_inherit_def(c, sync_new_items))
                    .collect(),
            )
        };

        MenuNodeDef::Inherit {
            from: clean.clone(),
            match_labels: vec![],
            match_aliases: vec![],
            origin_path: node.origin_path.clone(),
            label: Some(clean),
            icon: node.icon.clone(),
            enabled: node.enabled,
            visible: node.visible,
            shortcut: node.shortcut.clone(),
            sync_new_items,
            items: child_defs,
        }
    }
}
