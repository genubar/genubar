mod fetcher;

use crate::models::{
    AppReleaseManifest, AppUpdateInfo, ExtensionReleaseManifest, FetchedPackagePayload,
    InstalledDaemonInfo, InstalledExtensionInfo, RegistrySourceStatus, RemotePackageEntry,
    RemoteRegistryCatalog, UpdateCheckStatus,
};
use genubar_core::{CURRENT_APP_VERSION, EXTENSION_FILES, MenuError, is_version_compatible};
use genubar_platform::{
    expand_tilde, inspect_installed_extension as platform_inspect_installed_extension,
    install_embedded_extension, query_installed_daemon_version, write_downloaded_extension_files,
};
use std::fs;
use std::process::Command;

pub struct UpdateService;

impl UpdateService {
    pub fn fetch_bytes_from_source(url_or_path: &str) -> Result<Vec<u8>, MenuError> {
        let trimmed = url_or_path.trim();
        if trimmed.is_empty() {
            return Err(MenuError::Config(String::from("Empty URL or file path")));
        }

        if let Some(file_path) = trimmed.strip_prefix("file://") {
            let expanded = expand_tilde(file_path);
            return fs::read(&expanded).map_err(|e| {
                MenuError::IoPermission(format!("Failed to read {}: {e}", expanded.display()))
            });
        }

        if trimmed.starts_with('/') || trimmed.starts_with("~/") {
            let expanded = expand_tilde(trimmed);
            return fs::read(&expanded).map_err(|e| {
                MenuError::IoPermission(format!("Failed to read {}: {e}", expanded.display()))
            });
        }

        let out = Command::new("curl")
            .args(["-fsSL", "--max-time", "5", trimmed])
            .output()
            .map_err(|e| MenuError::Transport(format!("Failed to run curl: {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(MenuError::Transport(format!(
                "HTTP error for {trimmed}: {}",
                stderr.trim()
            )));
        }

        Ok(out.stdout)
    }

    pub fn resolve_relative_source(source_url: &str, target: &str) -> String {
        let t = target.trim();
        if t.contains("://") || t.starts_with('/') || t.starts_with("~/") {
            return t.to_string();
        }
        if let Some((base_dir, _)) = source_url.rsplit_once('/') {
            return format!("{base_dir}/{}", t.trim_start_matches("./"));
        }
        t.to_string()
    }

    // Checks dedicated `app.json` first, then falls back to `registry_urls` for backward compatibility.
    pub fn check_for_app_updates(
        app_update_url: &str,
        registry_urls: &[String],
    ) -> UpdateCheckStatus {
        let mut saw_reachable_manifest = false;
        let mut parsed_ext: Option<ExtensionReleaseManifest> = None;

        if !app_update_url.trim().is_empty()
            && let Ok(bytes) = Self::fetch_bytes_from_source(app_update_url)
                && let Ok(manifest) = serde_json::from_slice::<AppReleaseManifest>(&bytes) {
                    parsed_ext = manifest.extension.clone();
                    if !manifest.latest_app_version.trim().is_empty() {
                        if !is_version_compatible(&manifest.latest_app_version, CURRENT_APP_VERSION) {
                            return UpdateCheckStatus::UpdateAvailable(AppUpdateInfo {
                                version: manifest.latest_app_version,
                                download_url: manifest.download_url,
                                changelog: manifest.changelog,
                                extension: manifest.extension,
                            });
                        }
                        return UpdateCheckStatus::UpToDate {
                            extension: manifest.extension,
                        };
                    }
                    saw_reachable_manifest = true;
                }

        for reg_url in registry_urls {
            if let Ok(bytes) = Self::fetch_bytes_from_source(reg_url)
                && let Ok(cat) = serde_json::from_slice::<RemoteRegistryCatalog>(&bytes) {
                    saw_reachable_manifest = true;
                    if let Some(latest) = cat.latest_app_version
                        && !latest.trim().is_empty()
                            && !is_version_compatible(&latest, CURRENT_APP_VERSION)
                        {
                            return UpdateCheckStatus::UpdateAvailable(AppUpdateInfo {
                                version: latest,
                                download_url: cat.download_url,
                                changelog: Vec::new(),
                                extension: parsed_ext,
                            });
                        }
                }
        }

        if saw_reachable_manifest {
            UpdateCheckStatus::UpToDate {
                extension: parsed_ext,
            }
        } else {
            UpdateCheckStatus::OfflineOrUnreachable
        }
    }

    pub fn inspect_installed_daemon() -> InstalledDaemonInfo {
        let installed_ver = query_installed_daemon_version();

        let service_active = Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", "genubar.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        match installed_ver {
            None => InstalledDaemonInfo {
                installed: false,
                version: String::new(),
                service_active,
                status_summary: String::from(
                    "Daemon binary not found in ~/.local/bin — run genubar-installer to install",
                ),
            },
            Some(ver) => {
                let svc_state = if service_active {
                    "systemd user service running"
                } else {
                    "systemd user service stopped"
                };
                InstalledDaemonInfo {
                    installed: true,
                    version: ver,
                    service_active,
                    status_summary: format!("Installed in ~/.local/bin ({svc_state})"),
                }
            }
        }
    }

    pub fn install_local_or_embedded_extension() -> Result<String, MenuError> {
        install_embedded_extension(true)
    }

    pub fn inspect_installed_extension() -> InstalledExtensionInfo {
        platform_inspect_installed_extension()
    }

    pub fn update_extension_files(raw_base_url: &str) -> Result<String, MenuError> {
        let base = raw_base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err(MenuError::Config(String::from(
                "Extension raw_base_url is empty",
            )));
        }

        let mut downloaded = Vec::with_capacity(EXTENSION_FILES.len());
        for &fname in EXTENSION_FILES {
            let src = format!("{base}/{fname}");
            let bytes = Self::fetch_bytes_from_source(&src)?;
            downloaded.push((fname, bytes));
        }

        write_downloaded_extension_files(&downloaded)
    }

    pub fn fetch_remote_packages(
        registry_urls: &[String],
    ) -> (Vec<RemotePackageEntry>, Vec<RegistrySourceStatus>) {
        fetcher::fetch_remote_packages(registry_urls)
    }

    pub fn fetch_package_payload_ext(
        pkg: &RemotePackageEntry,
        allow_incompatible: bool,
    ) -> Result<FetchedPackagePayload, MenuError> {
        fetcher::fetch_package_payload_ext(pkg, allow_incompatible)
    }
}
