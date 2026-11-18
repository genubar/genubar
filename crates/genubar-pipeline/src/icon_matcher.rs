use genubar_config::{IconMatchRule, IconPathPattern};
use genubar_core::{ActiveWindowInfo, wildcard_match};
use std::path::Path;

pub struct IconEvalContext<'a> {
    pub info: &'a ActiveWindowInfo,
    pub parent_path: &'a [String],
    pub current_path: &'a [String],
    pub label: &'a str,
    pub action: Option<&'a str>,
    pub index_in_parent: usize,
    pub sibling_labels: &'a [String],
}

pub trait IconPathPatternExt {
    fn matches_path(&self, actual_path: &[String]) -> bool;
}

impl IconPathPatternExt for IconPathPattern {
    fn matches_path(&self, actual_path: &[String]) -> bool {
        let pattern_segments: Vec<&str> = match self {
            IconPathPattern::ArrowString(s) => s
                .split("->")
                .map(str::trim)
                .filter(|seg| !seg.is_empty())
                .collect(),
            IconPathPattern::Segments(vec) => vec.iter().map(String::as_str).collect(),
        };

        if pattern_segments.is_empty() || pattern_segments.len() != actual_path.len() {
            return false;
        }

        pattern_segments
            .iter()
            .zip(actual_path.iter())
            .all(|(pat, actual)| wildcard_match(pat, actual))
    }
}

pub trait IconMatchRuleExt {
    fn matches(&self, ctx: &IconEvalContext<'_>) -> bool;
    fn resolve_icon_value(&self, pack_dir: Option<&Path>) -> Option<String>;
}

impl IconMatchRuleExt for IconMatchRule {
    fn matches(&self, ctx: &IconEvalContext<'_>) -> bool {
        if is_excluded(self, ctx) || !matches_scope_and_position(self, ctx) {
            return false;
        }
        matches_positive_targets(self, ctx)
    }

    fn resolve_icon_value(&self, pack_dir: Option<&Path>) -> Option<String> {
        let trimmed = self.icon.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            return None;
        }
        if let Some(dir) = pack_dir {
            let low = trimmed.to_lowercase();
            if low.ends_with(".svg")
                || low.ends_with(".png")
                || low.ends_with(".jpg")
                || low.ends_with(".jpeg")
                || trimmed.contains('/')
            {
                let candidate = dir.join(trimmed);
                if candidate.exists() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
        Some(trimmed.to_string())
    }
}

fn is_excluded(rule: &IconMatchRule, ctx: &IconEvalContext<'_>) -> bool {
    if !rule.exclude_apps.is_empty()
        && rule
            .exclude_apps
            .iter()
            .any(|pat| wildcard_match(pat, &ctx.info.app_id) || wildcard_match(pat, &ctx.info.title))
    {
        return true;
    }

    if !rule.exclude_paths.is_empty()
        && rule
            .exclude_paths
            .iter()
            .any(|p| p.matches_path(ctx.current_path))
    {
        return true;
    }

    if !rule.exclude_labels.is_empty()
        && rule
            .exclude_labels
            .iter()
            .any(|pat| wildcard_match(pat, ctx.label))
    {
        return true;
    }

    if let Some(act) = ctx.action
        && !rule.exclude_actions.is_empty()
            && rule
                .exclude_actions
                .iter()
                .any(|pat| wildcard_match(pat, act))
        {
            return true;
        }

    false
}

fn matches_scope_and_position(rule: &IconMatchRule, ctx: &IconEvalContext<'_>) -> bool {
    if !rule.match_apps.is_empty() {
        let app_ok = rule.match_apps.iter().any(|pat| {
            wildcard_match(pat, &ctx.info.app_id) || wildcard_match(pat, &ctx.info.title)
        });
        if !app_ok {
            return false;
        }
    }

    if !rule.match_parent_paths.is_empty()
        && !rule
            .match_parent_paths
            .iter()
            .any(|p| p.matches_path(ctx.parent_path))
    {
        return false;
    }

    if let Some(min_i) = rule.min_index
        && ctx.index_in_parent < min_i {
            return false;
        }

    if let Some(max_i) = rule.max_index
        && ctx.index_in_parent > max_i {
            return false;
        }

    if let Some(ref after_pat) = rule.after_label {
        let anchor_pos = ctx
            .sibling_labels
            .iter()
            .position(|lbl| !lbl.is_empty() && wildcard_match(after_pat, lbl));
        if !matches!(anchor_pos, Some(pos) if ctx.index_in_parent > pos) {
            return false;
        }
    }

    if let Some(ref before_pat) = rule.before_label {
        let anchor_pos = ctx
            .sibling_labels
            .iter()
            .position(|lbl| !lbl.is_empty() && wildcard_match(before_pat, lbl));
        if !matches!(anchor_pos, Some(pos) if ctx.index_in_parent < pos) {
            return false;
        }
    }

    true
}

fn matches_positive_targets(rule: &IconMatchRule, ctx: &IconEvalContext<'_>) -> bool {
    let has_item_matchers = !rule.match_paths.is_empty()
        || !rule.match_labels.is_empty()
        || !rule.match_actions.is_empty();

    if has_item_matchers {
        if !rule.match_paths.is_empty()
            && rule
                .match_paths
                .iter()
                .any(|p| p.matches_path(ctx.current_path))
        {
            return true;
        }

        if rule
            .match_labels
            .iter()
            .any(|pat| wildcard_match(pat, ctx.label))
        {
            return true;
        }

        if let Some(act) = ctx.action
            && rule
                .match_actions
                .iter()
                .any(|pat| wildcard_match(pat, act))
            {
                return true;
            }

        return false;
    }

    !rule.match_parent_paths.is_empty()
        || rule.min_index.is_some()
        || rule.max_index.is_some()
        || rule.after_label.is_some()
        || rule.before_label.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn matches_paths_positions_exclusions_and_resolves_pack_icons() {
        let info = ActiveWindowInfo {
            app_id: String::from("org.gnome.TextEditor"),
            title: String::from("notes.txt"),
            ..Default::default()
        };
        let parent_path = vec![String::from("File")];
        let current_path = vec![String::from("File"), String::from("Save As")];
        let siblings = vec![
            String::from("New Tab"),
            String::from("Save"),
            String::from("Save As"),
            String::from("Quit"),
        ];

        let ctx = IconEvalContext {
            info: &info,
            parent_path: &parent_path,
            current_path: &current_path,
            label: "Save As",
            action: Some("win.save-as"),
            index_in_parent: 2,
            sibling_labels: &siblings,
        };

        let matching_rule = IconMatchRule {
            icon: String::from("icons/save-as.svg"),
            match_apps: vec![String::from("*TextEditor*")],
            match_paths: vec![IconPathPattern::ArrowString(String::from("File -> Save*"))],
            after_label: Some(String::from("Save")),
            before_label: Some(String::from("Quit")),
            min_index: Some(1),
            max_index: Some(3),
            ..Default::default()
        };
        assert!(matching_rule.matches(&ctx));

        let excluded_rule = IconMatchRule {
            exclude_labels: vec![String::from("Save As")],
            ..matching_rule.clone()
        };
        assert!(!excluded_rule.matches(&ctx));

        let suppress_rule = IconMatchRule {
            icon: String::from("none"),
            ..Default::default()
        };
        assert_eq!(suppress_rule.resolve_icon_value(None), None);

        let pack_dir = std::env::temp_dir().join("genubar_test_icon_matcher_pack");
        let _ = fs::remove_dir_all(&pack_dir);
        fs::create_dir_all(pack_dir.join("icons")).unwrap();
        fs::write(pack_dir.join("icons/save-as.svg"), "<svg/>").unwrap();

        let resolved = matching_rule
            .resolve_icon_value(Some(&pack_dir))
            .expect("pack relative svg should resolve");
        assert!(resolved.ends_with("icons/save-as.svg"));

        let _ = fs::remove_dir_all(&pack_dir);
    }
}
