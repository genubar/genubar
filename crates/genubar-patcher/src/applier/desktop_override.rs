use super::StatefulStepApplier;
use crate::utils::paths::expand_tokens;
use genubar_core::MenuError;
use genubar_platform::desktop_probe::{locate_desktop_file, read_dbus_service_exec};
use genubar_platform::fs_utils::{read_text_file, remove_file_if_exists, write_text_file};
use genubar_platform::user_applications_dir;
use std::collections::BTreeMap;

pub(crate) struct DesktopOverrideApplier<'a> {
    pub desktop_file: &'a str,
    pub aliases: &'a [String],
    pub inject_env: &'a BTreeMap<String, String>,
    pub append_args: &'a [String],
    pub force_no_dbus_activatable: bool,
}

impl StatefulStepApplier for DesktopOverrideApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("desktop:{}", self.desktop_file),
            format!("env={:?};args={:?}", self.inject_env, self.append_args),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(
            self.desktop_file,
            self.aliases,
            self.inject_env,
            self.append_args,
            self.force_no_dbus_activatable,
        )
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.desktop_file, self.aliases, orig_opt)
    }
}

pub fn apply(
    desktop_file: &str,
    aliases: &[String],
    inject_env: &BTreeMap<String, String>,
    append_args: &[String],
    force_no_dbus_activatable: bool,
) -> Result<Option<String>, MenuError> {
    let apps_dir = user_applications_dir();
    let local_dest = apps_dir.join(desktop_file);

    let orig_local_content = if local_dest.is_file() {
        Some(read_text_file(&local_dest)?)
    } else {
        None
    };

    let mut candidates: Vec<&str> = Vec::with_capacity(1 + aliases.len());
    candidates.push(desktop_file);
    for alias in aliases {
        candidates.push(alias.as_str());
    }

    let source_path = locate_desktop_file(&candidates).ok_or_else(|| {
        MenuError::NotFound(format!("System .desktop file '{desktop_file}' not found"))
    })?;

    let raw_source = read_text_file(&source_path)?;
    let transformed = transform_desktop_content(
        &raw_source,
        inject_env,
        append_args,
        force_no_dbus_activatable,
    );

    write_text_file(&local_dest, &transformed)?;
    for alias in aliases {
        if alias != desktop_file && locate_desktop_file(&[alias.as_str()]).is_some() {
            write_text_file(apps_dir.join(alias), &transformed)?;
        }
    }
    Ok(orig_local_content)
}

pub fn revert(
    desktop_file: &str,
    aliases: &[String],
    orig_opt: Option<String>,
) -> Result<(), MenuError> {
    let apps_dir = user_applications_dir();
    let local_path = apps_dir.join(desktop_file);
    match orig_opt {
        Some(prev_content) => write_text_file(&local_path, &prev_content)?,
        None => {
            remove_file_if_exists(&local_path)?;
            for alias in aliases {
                if alias != desktop_file {
                    remove_file_if_exists(apps_dir.join(alias))?;
                }
            }
        }
    }
    Ok(())
}

fn transform_desktop_content(
    raw: &str,
    inject_env: &BTreeMap<String, String>,
    append_args: &[String],
    force_no_dbus: bool,
) -> String {
    let mut env_pairs: Vec<String> = inject_env
        .iter()
        .map(|(k, v)| format!("{k}=\"{}\"", expand_tokens(v)))
        .collect();
    env_pairs.sort();
    let env_prefix = if env_pairs.is_empty() {
        String::new()
    } else {
        format!("env {} ", env_pairs.join(" "))
    };

    let args_suffix = if append_args.is_empty() {
        String::new()
    } else {
        let expanded: Vec<String> = append_args.iter().map(|a| expand_tokens(a)).collect();
        format!(" {}", expanded.join(" "))
    };

    let mut out_lines = Vec::new();
    let mut seen_dbus_activatable = false;
    let mut in_desktop_entry = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_desktop_entry && force_no_dbus && !seen_dbus_activatable {
                out_lines.push(String::from("DBusActivatable=false"));
                seen_dbus_activatable = true;
            }
            in_desktop_entry = trimmed == "[Desktop Entry]";
            out_lines.push(line.to_string());
            continue;
        }

        if let Some(exec_cmd) = line.strip_prefix("Exec=") {
            let stripped_cmd = strip_existing_env_prefix(exec_cmd.trim_start(), inject_env);
            let resolved_cmd = resolve_gapplication_launch_cmd(stripped_cmd);
            let suffix = if in_desktop_entry
                && !args_suffix.is_empty()
                && !resolved_cmd.contains(args_suffix.trim())
            {
                &args_suffix
            } else {
                ""
            };
            out_lines.push(format!("Exec={env_prefix}{resolved_cmd}{suffix}"));
        } else if in_desktop_entry && force_no_dbus && line.starts_with("DBusActivatable=") {
            out_lines.push(String::from("DBusActivatable=false"));
            seen_dbus_activatable = true;
        } else {
            out_lines.push(line.to_string());
        }
    }

    if force_no_dbus && !seen_dbus_activatable {
        out_lines.push(String::from("DBusActivatable=false"));
    }

    out_lines.join("\n") + "\n"
}

fn strip_existing_env_prefix<'a>(
    mut cmd: &'a str,
    inject_env: &BTreeMap<String, String>,
) -> &'a str {
    while let Some(after_env) = cmd.strip_prefix("env ") {
        let mut rest = after_env.trim_start();
        let mut stripped_any = false;
        while let Some(eq_idx) = rest.find('=') {
            let key = &rest[..eq_idx];
            if key.is_empty() || key.contains(char::is_whitespace) || !inject_env.contains_key(key)
            {
                break;
            }
            let after_eq = &rest[eq_idx + 1..];
            if let Some(after_quote) = after_eq.strip_prefix('"') {
                if let Some(end_q) = after_quote.find('"') {
                    rest = after_quote[end_q + 1..].trim_start();
                    stripped_any = true;
                    continue;
                }
            } else if let Some(ws_idx) = after_eq.find(char::is_whitespace) {
                rest = after_eq[ws_idx..].trim_start();
                stripped_any = true;
                continue;
            }
            break;
        }
        if stripped_any {
            cmd = rest;
        } else {
            break;
        }
    }
    cmd
}

// Some GNOME apps (`org.gnome.Weather.desktop`, `org.gnome.Maps.desktop`) set
// `Exec=gapplication launch <app_id> [%U]` instead of invoking their binary directly.
// Because `gapplication launch` only sends a D-Bus activation message and exits immediately,
// `LD_PRELOAD` on `gapplication` never reaches the actual app process unless we resolve the
// real executable from `/usr/share/dbus-1/services/<app_id>.service`.
fn resolve_gapplication_launch_cmd(cmd: &str) -> String {
    let Some(rest) = cmd.strip_prefix("gapplication launch ") else {
        return cmd.to_string();
    };
    let mut parts = rest.split_whitespace();
    let Some(app_id) = parts.next() else {
        return cmd.to_string();
    };
    let trailing_args: Vec<&str> = parts.collect();

    let Some(bin_cmd) = read_dbus_service_exec(app_id) else {
        return cmd.to_string();
    };
    if trailing_args.is_empty() {
        bin_cmd
    } else {
        format!("{bin_cmd} {}", trailing_args.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_dbus_activatable_before_desktop_actions_and_is_idempotent() {
        let raw = "[Desktop Entry]\nName=Papers\nExec=papers %U\nActions=new-window;\n\n[Desktop Action new-window]\nName=New Window\nExec=papers\n";
        let mut env = BTreeMap::new();
        env.insert(String::from("LD_PRELOAD"), String::from("/tmp/hook.so"));

        let once = transform_desktop_content(raw, &env, &[], true);
        let twice = transform_desktop_content(&once, &env, &[], true);
        assert_eq!(once, twice);

        let action_idx = once.find("[Desktop Action new-window]").unwrap();
        let dbus_idx = once.find("DBusActivatable=false").unwrap();
        assert!(
            dbus_idx < action_idx,
            "DBusActivatable=false must appear inside [Desktop Entry] before [Desktop Action]:\n{once}"
        );
    }
}