use crate::models::{
    FetchedPackagePayload, RegistrySourceStatus, RemotePackageEntry, RemoteRegistryCatalog,
};
use crate::update::UpdateService;
use genubar_core::{MenuError, PackageKind};
use genubar_platform::expand_tilde;
use std::collections::HashMap;

// Evaluates every configured registry source independently so one unreachable URL never blocks valid sources.
pub fn fetch_remote_packages(
    registry_urls: &[String],
) -> (Vec<RemotePackageEntry>, Vec<RegistrySourceStatus>) {
    let mut entries = Vec::new();
    let mut statuses = Vec::new();

    for url in registry_urls {
        match UpdateService::fetch_bytes_from_source(url) {
            Ok(bytes) => match serde_json::from_slice::<RemoteRegistryCatalog>(&bytes) {
                Ok(cat) => {
                    let count = cat.entries.len();
                    for mut pkg in cat.entries {
                        pkg.source_url = url.clone();
                        entries.push(pkg);
                    }
                    statuses.push(RegistrySourceStatus {
                        source: url.clone(),
                        loaded_count: count,
                        error: None,
                    });
                }
                Err(e) => statuses.push(RegistrySourceStatus {
                    source: url.clone(),
                    loaded_count: 0,
                    error: Some(format!("Invalid registry JSON: {e}")),
                }),
            },
            Err(e) => statuses.push(RegistrySourceStatus {
                source: url.clone(),
                loaded_count: 0,
                error: Some(e.to_string()),
            }),
        }
    }

    (entries, statuses)
}

pub fn fetch_package_payload_ext(
    pkg: &RemotePackageEntry,
    allow_incompatible: bool,
) -> Result<FetchedPackagePayload, MenuError> {
    if !allow_incompatible && !pkg.is_compatible() {
        return Err(MenuError::Config(
            pkg.incompatibility_badge()
                .unwrap_or_else(|| String::from("Package requires a newer Genubar version")),
        ));
    }

    if pkg.kind == PackageKind::Bundle
        && pkg.files.is_empty()
        && let Some(ref raw_url) = pkg.download_url
    {
        let resolved = UpdateService::resolve_relative_source(&pkg.source_url, raw_url);
        let expanded = if let Some(p) = resolved.strip_prefix("file://") {
            expand_tilde(p)
        } else {
            expand_tilde(&resolved)
        };
        if expanded.is_dir() {
            return Ok(FetchedPackagePayload::LocalBundleDir(expanded));
        }
    }

    if !pkg.files.is_empty() || (pkg.kind == PackageKind::Bundle && pkg.content.is_some()) {
        let mut materialized = HashMap::new();
        if let Some(ref root_toml) = pkg.content {
            let default_name = match pkg.kind {
                PackageKind::Bundle => "bundle.toml",
                PackageKind::IconPack => "pack.toml",
                _ => "manifest.toml",
            };
            materialized.insert(String::from(default_name), root_toml.clone());
        }
        for (rel_path, content_or_url) in &pkg.files {
            let trimmed = content_or_url.trim();
            let body = if trimmed.starts_with("http://")
                || trimmed.starts_with("https://")
                || trimmed.starts_with("file://")
                || trimmed.starts_with("./")
                || trimmed.starts_with('/')
                || trimmed.starts_with("~/")
            {
                let resolved = UpdateService::resolve_relative_source(&pkg.source_url, trimmed);
                let bytes = UpdateService::fetch_bytes_from_source(&resolved)?;
                String::from_utf8(bytes).map_err(|e| {
                    MenuError::ParseError(format!("Non-UTF8 file for {rel_path}: {e}"))
                })?
            } else {
                content_or_url.clone()
            };
            materialized.insert(rel_path.clone(), body);
        }
        return Ok(FetchedPackagePayload::BundleFiles {
            files: materialized,
        });
    }

    if let Some(ref raw) = pkg.content {
        let ext = if pkg.kind == PackageKind::Style {
            "css"
        } else {
            "toml"
        };
        let hint = pkg
            .download_url
            .as_deref()
            .and_then(|u| u.rsplit('/').next())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.{ext}", pkg.id));
        return Ok(FetchedPackagePayload::TextContent {
            raw: raw.clone(),
            file_name_hint: hint,
        });
    }

    let Some(ref raw_url) = pkg.download_url else {
        return Err(MenuError::NotFound(format!(
            "Package '{}' has no content or download_url",
            pkg.id
        )));
    };
    let resolved = UpdateService::resolve_relative_source(&pkg.source_url, raw_url);
    let bytes = UpdateService::fetch_bytes_from_source(&resolved)?;
    let raw = String::from_utf8(bytes).map_err(|e| {
        MenuError::ParseError(format!(
            "Downloaded package '{}' is not valid UTF-8: {e}",
            pkg.id
        ))
    })?;
    let hint = resolved
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}.toml", pkg.id));
    Ok(FetchedPackagePayload::TextContent {
        raw,
        file_name_hint: hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn fetch_remote_packages_keeps_valid_sources_when_another_source_fails() {
        let tmp_json = std::env::temp_dir().join("genubar_test_valid_registry.json");
        fs::write(
            &tmp_json,
            genubar_config::defaults::DEFAULT_EXAMPLE_REGISTRY_JSON,
        )
        .unwrap();

        let missing = String::from("/tmp/definitely_missing_genubar_registry_99999.json");
        let valid = tmp_json.to_string_lossy().to_string();

        let (entries, statuses) = UpdateService::fetch_remote_packages(&[missing, valid]);
        let _ = fs::remove_file(&tmp_json);

        assert_eq!(entries.len(), 4);
        assert_eq!(statuses.len(), 2);
        assert!(statuses[0].error.is_some());
        assert_eq!(statuses[0].loaded_count, 0);
        assert!(statuses[1].error.is_none());
        assert_eq!(statuses[1].loaded_count, 4);
    }
}
