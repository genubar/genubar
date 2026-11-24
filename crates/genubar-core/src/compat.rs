pub use genubar_protocol::{EXTENSION_FILES, EXTENSION_UUID};
use serde::Deserialize;

pub const CURRENT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const EXPECTED_EXTENSION_VERSION: &str = env!("GENUBAR_EXPECTED_EXTENSION_VERSION");
pub const MIN_COMPATIBLE_EXTENSION_VERSION: &str = "0.1.0";
pub const IPC_PROTOCOL_VERSION: u32 = 1;

pub fn parse_semver_triple(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split('-').next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Returns `true` if `current` satisfies `>= required` under SemVer major.minor.patch ordering.
pub fn is_version_compatible(required: &str, current: &str) -> bool {
    match (parse_semver_triple(required), parse_semver_triple(current)) {
        (Some(req), Some(cur)) => cur >= req,
        _ => true,
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionCompatReport {
    pub installed: bool,
    pub version: String,
    pub min_daemon_version: String,
    pub tested_daemon_version: String,
    pub ipc_protocol_version: u32,
    pub is_compatible: bool,
    pub status_summary: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RawExtensionMetadataJson {
    #[serde(rename = "genubar-version")]
    genubar_version: Option<String>,
    #[serde(rename = "version-name")]
    version_name: Option<String>,
    #[serde(rename = "min-daemon-version")]
    min_daemon_version: Option<String>,
    #[serde(rename = "tested-daemon-version")]
    tested_daemon_version: Option<String>,
    #[serde(rename = "ipc-protocol-version")]
    ipc_protocol_version: Option<u32>,
    version: Option<u64>,
}

/// Validates a GNOME Shell extension `metadata.json` against current daemon & IPC protocol versions.
pub fn check_extension_metadata_json(raw: Option<&str>) -> ExtensionCompatReport {
    let Some(raw_text) = raw else {
        return ExtensionCompatReport {
            installed: false,
            version: String::from("0.0.0"),
            min_daemon_version: String::new(),
            tested_daemon_version: String::new(),
            ipc_protocol_version: 0,
            is_compatible: false,
            status_summary: format!(
                "Extension {EXTENSION_UUID} not found in ~/.local/share/gnome-shell/extensions/{EXTENSION_UUID} (Daemon expects v{EXPECTED_EXTENSION_VERSION})"
            ),
        };
    };

    let Ok(parsed) = serde_json::from_str::<RawExtensionMetadataJson>(raw_text) else {
        return ExtensionCompatReport {
            installed: true,
            version: String::from("unknown"),
            min_daemon_version: String::new(),
            tested_daemon_version: String::new(),
            ipc_protocol_version: 0,
            is_compatible: false,
            status_summary: format!(
                "⚠ Invalid metadata.json in {EXTENSION_UUID} (Daemon v{CURRENT_APP_VERSION} expects extension v{EXPECTED_EXTENSION_VERSION})"
            ),
        };
    };

    let ext_ver = parsed
        .genubar_version
        .or(parsed.version_name)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            if parsed.version.unwrap_or(0) >= 1 {
                String::from("0.1.0")
            } else {
                String::from("0.0.0")
            }
        });
    let min_daemon = parsed
        .min_daemon_version
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| String::from("0.1.0"));
    let tested_daemon = parsed
        .tested_daemon_version
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| String::from("0.1.0"));
    let ext_ipc = parsed.ipc_protocol_version.unwrap_or(IPC_PROTOCOL_VERSION);

    let (is_compatible, status_summary) = if ext_ipc != IPC_PROTOCOL_VERSION {
        (
            false,
            format!(
                "⚠ Incompatible IPC protocol: Extension uses v{ext_ipc}, Daemon v{CURRENT_APP_VERSION} uses v{IPC_PROTOCOL_VERSION} (expected extension v{EXPECTED_EXTENSION_VERSION})"
            ),
        )
    } else if !is_version_compatible(&min_daemon, CURRENT_APP_VERSION) {
        (
            false,
            format!(
                "⚠ Incompatible: Extension v{ext_ver} requires Daemon v{min_daemon}+ (current Daemon v{CURRENT_APP_VERSION} was designed for extension v{EXPECTED_EXTENSION_VERSION})"
            ),
        )
    } else if !is_version_compatible(MIN_COMPATIBLE_EXTENSION_VERSION, &ext_ver) {
        (
            false,
            format!(
                "⚠ Incompatible: Daemon v{CURRENT_APP_VERSION} requires Extension v{MIN_COMPATIBLE_EXTENSION_VERSION}+ (designed for v{EXPECTED_EXTENSION_VERSION}, installed v{ext_ver})"
            ),
        )
    } else {
        (
            true,
            format!(
                "Compatible with Daemon v{CURRENT_APP_VERSION} (designed for extension v{EXPECTED_EXTENSION_VERSION}, IPC v{ext_ipc})"
            ),
        )
    };

    ExtensionCompatReport {
        installed: true,
        version: ext_ver,
        min_daemon_version: min_daemon,
        tested_daemon_version: tested_daemon,
        ipc_protocol_version: ext_ipc,
        is_compatible,
        status_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compatibility_checks_work() {
        assert!(parse_semver_triple(EXPECTED_EXTENSION_VERSION).is_some());
        assert!(is_version_compatible("0.1.0", "0.1.0"));
        assert!(is_version_compatible("0.1.0", "v0.2.0"));
        assert!(!is_version_compatible("0.2.0", "0.1.9"));
        assert!(!is_version_compatible("1.0.0", "0.99.0"));
    }

    #[test]
    fn metadata_json_compatibility_reports_clear_states() {
        let missing = check_extension_metadata_json(None);
        assert!(!missing.installed);
        assert!(!missing.is_compatible);

        let valid_json = r#"{
            "uuid": "genubar@github.com",
            "genubar-version": "0.1.0",
            "min-daemon-version": "0.1.0",
            "ipc-protocol-version": 1
        }"#;
        let ok = check_extension_metadata_json(Some(valid_json));
        assert!(ok.installed);
        assert!(ok.is_compatible);
        assert_eq!(ok.version, "0.1.0");

        let wrong_ipc = r#"{"genubar-version": "0.1.0", "ipc-protocol-version": 99}"#;
        let bad_ipc = check_extension_metadata_json(Some(wrong_ipc));
        assert!(!bad_ipc.is_compatible);

        let needs_newer = r#"{"genubar-version": "0.9.0", "min-daemon-version": "99.0.0", "ipc-protocol-version": 1}"#;
        let bad_daemon = check_extension_metadata_json(Some(needs_newer));
        assert!(!bad_daemon.is_compatible);
    }
}
