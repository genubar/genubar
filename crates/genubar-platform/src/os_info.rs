use std::fs;
use std::sync::OnceLock;

static DETECTED_OS_NAME: OnceLock<String> = OnceLock::new();

/// Parses the OS display name from `/etc/os-release` contents.
pub fn parse_os_name(os_release_raw: &str) -> Option<String> {
    let mut name: Option<String> = None;
    let mut id: Option<String> = None;
    let mut pretty_name: Option<String> = None;

    for raw_line in os_release_raw.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let cleaned = val.trim().trim_matches('"').trim_matches('\'').trim();
        if cleaned.is_empty() {
            continue;
        }
        match key.trim() {
            "NAME" => name = Some(cleaned.to_string()),
            "PRETTY_NAME" => pretty_name = Some(cleaned.to_string()),
            "ID" => id = Some(cleaned.to_string()),
            _ => {}
        }
    }

    if let Some(n) = name {
        let short = n.strip_suffix(" Linux").unwrap_or(&n).trim();
        if !short.is_empty() {
            return Some(short.to_string());
        }
        return Some(n);
    }

    if let Some(pretty) = pretty_name {
        return Some(pretty);
    }

    id.map(|raw_id| {
        let mut chars = raw_id.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::from("Linux"),
        }
    })
}

/// Returns the cached host OS name (read once from `/etc/os-release`), falling back to `"Linux"`.
pub fn detect_os_name() -> &'static str {
    DETECTED_OS_NAME
        .get_or_init(|| {
            fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|raw| parse_os_name(&raw))
                .unwrap_or_else(|| String::from("Linux"))
        })
        .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_various_os_release_files_and_falls_back() {
        let fedora = r#"
NAME="Fedora Linux"
VERSION="41 (Workstation Edition)"
ID=fedora
"#;
        assert_eq!(parse_os_name(fedora).as_deref(), Some("Fedora"));

        let ubuntu = r#"
PRETTY_NAME="Ubuntu 24.04 LTS"
NAME="Ubuntu"
ID=ubuntu
"#;
        assert_eq!(parse_os_name(ubuntu).as_deref(), Some("Ubuntu"));

        let arch = r#"
NAME="Arch Linux"
ID=arch
"#;
        assert_eq!(parse_os_name(arch).as_deref(), Some("Arch"));

        let only_id = "ID=nixos\n";
        assert_eq!(parse_os_name(only_id).as_deref(), Some("Nixos"));

        assert_eq!(parse_os_name(""), None);
        assert!(!detect_os_name().is_empty());
    }
}
