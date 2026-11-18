use super::StatefulStepApplier;
use genubar_core::MenuError;
use genubar_platform::fs_utils::{read_text_file, write_text_file};
use genubar_platform::home_dir;
use std::fs;
use std::path::PathBuf;

pub(crate) struct FirefoxPrefApplier<'a> {
    pub pref_key: &'a str,
    pub pref_value: &'a str,
}

impl StatefulStepApplier for FirefoxPrefApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("firefox_pref:{}", self.pref_key),
            self.pref_value.to_string(),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(self.pref_key, self.pref_value)
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.pref_key, orig_opt)
    }
}

pub fn apply(pref_key: &str, pref_value: &str) -> Result<Option<String>, MenuError> {
    let profile_dirs = discover_firefox_profile_dirs();
    if profile_dirs.is_empty() {
        return Err(MenuError::NotFound(
            "No Mozilla Firefox profile directories found".into(),
        ));
    }

    let needle = format!("user_pref(\"{pref_key}\"");
    let new_line = format!("user_pref(\"{pref_key}\", {pref_value});");
    let mut previous_line: Option<String> = None;

    for prof_dir in profile_dirs {
        let user_js = prof_dir.join("user.js");
        let existing = if user_js.is_file() {
            read_text_file(&user_js)?
        } else {
            String::new()
        };

        let mut lines: Vec<String> = Vec::new();
        for l in existing.lines() {
            if l.contains(&needle) {
                if previous_line.is_none() {
                    previous_line = Some(l.to_string());
                }
            } else {
                lines.push(l.to_string());
            }
        }

        lines.push(new_line.clone());
        write_text_file(&user_js, &(lines.join("\n") + "\n"))?;
    }

    Ok(previous_line)
}

pub fn revert(pref_key: &str, orig_line_opt: Option<String>) -> Result<(), MenuError> {
    let needle = format!("user_pref(\"{pref_key}\"");
    for prof_dir in discover_firefox_profile_dirs() {
        let user_js = prof_dir.join("user.js");
        if user_js.is_file() {
            let existing = read_text_file(&user_js)?;
            let mut filtered: Vec<String> = existing
                .lines()
                .filter(|l| !l.contains(&needle))
                .map(String::from)
                .collect();

            if let Some(ref prev_line) = orig_line_opt {
                filtered.push(prev_line.clone());
            }

            let new_body = if filtered.is_empty() {
                String::new()
            } else {
                filtered.join("\n") + "\n"
            };
            write_text_file(&user_js, &new_body)?;
        }
    }
    Ok(())
}

fn discover_firefox_profile_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let base_roots = [
        home.join(".mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
    ];

    let mut profiles = Vec::new();
    for root in base_roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && (p.join("prefs.js").is_file() || p.join("user.js").is_file()) {
                profiles.push(p);
            }
        }
    }
    profiles
}