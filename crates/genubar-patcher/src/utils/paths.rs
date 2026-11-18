use genubar_core::wildcard_match;
use std::fs;
use std::path::{Path, PathBuf};

pub use genubar_platform::home_dir;

/// Expands a leading `~/` and substitutes `{HOME}` and `{GTK4_HOOK_SO}` placeholders.
pub fn expand_path(raw: &Path) -> PathBuf {
    let raw_str = raw.to_string_lossy();
    let expanded_tilde = genubar_platform::expand_tilde(&raw_str);
    PathBuf::from(expand_tokens(&expanded_tilde.to_string_lossy()))
}

pub fn expand_tokens(input: &str) -> String {
    let home = home_dir();
    let hook_path = locate_gtk4_hook_so();
    input
        .replace("{HOME}", &home.to_string_lossy())
        .replace("{GTK4_HOOK_SO}", &hook_path)
}

pub fn locate_gtk4_hook_so() -> String {
    genubar_platform::locate_or_sync_gtk4_hook_so()
}

/// Resolves single-level wildcard patterns in file paths (e.g. `~/.config/JetBrains/RustRover*/rustrover64.vmoptions`).
pub fn resolve_file_pattern(pattern: &str) -> Vec<PathBuf> {
    let expanded = expand_path(Path::new(pattern));
    let expanded_str = expanded.to_string_lossy();

    if !expanded_str.contains('*') {
        return vec![expanded];
    }

    let mut matched_paths = Vec::new();
    let components: Vec<&str> = expanded_str.split('/').collect();
    resolve_segments_recursive(PathBuf::from("/"), &components[1..], &mut matched_paths);
    matched_paths
}

fn resolve_segments_recursive(current_base: PathBuf, remaining: &[&str], out: &mut Vec<PathBuf>) {
    let Some((first, rest)) = remaining.split_first() else {
        out.push(current_base);
        return;
    };

    if !first.contains('*') {
        resolve_segments_recursive(current_base.join(first), rest, out);
        return;
    }

    let Ok(entries) = fs::read_dir(&current_base) else {
        return;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if wildcard_match(first, &name_str) {
            resolve_segments_recursive(entry.path(), rest, out);
        }
    }
}