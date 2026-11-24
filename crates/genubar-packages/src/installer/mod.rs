mod search;

use crate::bundles::BundleManager;
use crate::models::{CatalogSearchItem, FetchedPackagePayload, RemotePackageEntry};
use genubar_config::{AppProfile, ConfigManager, IconPackManifest, StylesManager};
use genubar_core::{MenuError, PackageKind};
use genubar_patcher::{PackageOrigin, PatchManager, PatchRepository};
use genubar_platform::{fs_utils, parse_metadata_block};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct RegistryPackageInstaller;

impl RegistryPackageInstaller {
    fn validate_safe_id(id: &str, label: &str) -> Result<(), MenuError> {
        if id.trim().is_empty()
            || id != id.trim()
            || id.contains('/')
            || id.contains('\\')
            || id.contains('\0')
        {
            return Err(MenuError::Config(format!(
                "Invalid {label} '{id}': empty names and path separators are not allowed"
            )));
        }
        let mut comps = Path::new(id).components();
        match (comps.next(), comps.next()) {
            (Some(std::path::Component::Normal(_)), None) => Ok(()),
            _ => Err(MenuError::Config(format!(
                "Invalid {label} '{id}': unsafe path component"
            ))),
        }
    }

    fn validate_safe_relative_path(rel: &str) -> Result<PathBuf, MenuError> {
        if rel.trim().is_empty() || rel.contains('\\') || rel.contains('\0') {
            return Err(MenuError::Config(format!(
                "Unsafe archive relative path '{rel}'"
            )));
        }
        let mut safe_rel = PathBuf::new();
        for comp in Path::new(rel).components() {
            match comp {
                std::path::Component::Normal(part) => safe_rel.push(part),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    return Err(MenuError::Config(format!(
                        "Path traversal rejected in package file '{rel}'"
                    )));
                }
            }
        }
        if safe_rel.as_os_str().is_empty() {
            return Err(MenuError::Config(format!(
                "Empty relative path rejected in package file '{rel}'"
            )));
        }
        Ok(safe_rel)
    }

    fn write_safe_relative_files(
        base_dir: &Path,
        files: &HashMap<String, String>,
    ) -> Result<(), MenuError> {
        for (rel, body) in files {
            let safe_rel = Self::validate_safe_relative_path(rel)?;
            fs_utils::write_text_file(base_dir.join(safe_rel), body)?;
        }
        Ok(())
    }

    pub fn install_fetched_package_ext(
        cfg: &mut ConfigManager,
        patcher: &mut PatchManager,
        pkg: &RemotePackageEntry,
        payload: FetchedPackagePayload,
        allow_incompatible: bool,
    ) -> Result<String, MenuError> {
        Self::validate_safe_id(&pkg.id, "package id")?;
        if !allow_incompatible && !pkg.is_compatible() {
            return Err(MenuError::Config(
                pkg.incompatibility_badge()
                    .unwrap_or_else(|| String::from("Package requires a newer Genubar version")),
            ));
        }

        match (pkg.kind, payload) {
            (PackageKind::Patch, FetchedPackagePayload::TextContent { raw, .. }) => {
                Self::install_patch_text(patcher, pkg, &raw, allow_incompatible)
            }
            (PackageKind::Profile, FetchedPackagePayload::TextContent { raw, .. }) => {
                let mut profile: AppProfile = toml::from_str(&raw)
                    .map_err(|e| MenuError::ParseError(format!("Invalid profile TOML: {e}")))?;
                Self::validate_safe_id(&profile.id, "profile id")?;
                if !allow_incompatible && !profile.is_compatible() {
                    return Err(MenuError::Config(format!(
                        "Profile requires Genubar v{}",
                        profile.min_app_version.as_deref().unwrap_or("newer")
                    )));
                }
                if allow_incompatible && !profile.is_compatible() {
                    profile.enabled = false;
                }
                let name = profile.name.clone();
                cfg.save_profile(profile)?;
                Ok(format!("Saved profile '{name}'"))
            }
            (
                PackageKind::Style,
                FetchedPackagePayload::TextContent {
                    raw,
                    file_name_hint,
                },
            ) => {
                let fname = if file_name_hint.ends_with(".css") {
                    Self::validate_safe_id(&file_name_hint, "style file name")?;
                    file_name_hint
                } else {
                    format!("{}.css", pkg.id)
                };
                let meta = StylesManager::parse_header(&fname, &raw);
                if !allow_incompatible && !meta.is_compatible() {
                    return Err(MenuError::Config(format!(
                        "Style requires Genubar v{}",
                        meta.min_app_version.as_deref().unwrap_or("newer")
                    )));
                }
                let dst = cfg.styles.styles_dir().join(&fname);
                fs_utils::write_text_file(&dst, &raw)?;
                Ok(format!("Saved style '{}' ({fname})", meta.name))
            }
            (PackageKind::Script, FetchedPackagePayload::TextContent { raw, .. }) => {
                if !allow_incompatible
                    && parse_metadata_block(&raw, "# ==GenubarScript==", "# ==/GenubarScript==")
                        .is_none()
                {
                    return Err(MenuError::Config(String::from(
                        "Python script is missing mandatory '# ==GenubarScript==' header",
                    )));
                }
                let dst = cfg
                    .base_dir()
                    .join("scripts")
                    .join(format!("{}.py", pkg.id));
                fs_utils::write_text_file(&dst, &raw)?;
                Ok(format!("Saved script '{}' ({}.py)", pkg.name, pkg.id))
            }
            (PackageKind::IconPack, FetchedPackagePayload::TextContent { raw, .. }) => {
                let mut files = HashMap::new();
                files.insert(String::from("pack.toml"), raw);
                Self::install_icon_pack_files(cfg, pkg, &files, allow_incompatible)
            }
            (PackageKind::IconPack, FetchedPackagePayload::BundleFiles { files }) => {
                Self::install_icon_pack_files(cfg, pkg, &files, allow_incompatible)
            }
            (PackageKind::Bundle, FetchedPackagePayload::LocalBundleDir(dir)) => {
                let base_dir = cfg.base_dir().to_path_buf();
                let packages_dir = patcher.packages_dir.clone();
                let id = BundleManager::new(base_dir.clone())
                    .install_bundle_from_dir_ext(&dir, allow_incompatible)?;
                *cfg = ConfigManager::load_from_base_dir(base_dir)?;
                *patcher = PatchManager::load_from_packages_dir(packages_dir)?;
                Ok(format!("Installed bundle '{id}'"))
            }
            (PackageKind::Bundle, FetchedPackagePayload::BundleFiles { files }) => {
                let tmp_dir = std::env::temp_dir().join(format!("genubar_remote_{}", pkg.id));
                let _ = fs::remove_dir_all(&tmp_dir);
                if let Err(e) = Self::write_safe_relative_files(&tmp_dir, &files) {
                    let _ = fs::remove_dir_all(&tmp_dir);
                    return Err(e);
                }
                let base_dir = cfg.base_dir().to_path_buf();
                let packages_dir = patcher.packages_dir.clone();
                let res = BundleManager::new(base_dir.clone())
                    .install_bundle_from_dir_ext(&tmp_dir, allow_incompatible);
                let _ = fs::remove_dir_all(&tmp_dir);
                let id = res?;
                *cfg = ConfigManager::load_from_base_dir(base_dir)?;
                *patcher = PatchManager::load_from_packages_dir(packages_dir)?;
                Ok(format!("Installed bundle '{id}'"))
            }
            (
                PackageKind::Unknown,
                FetchedPackagePayload::TextContent {
                    raw,
                    file_name_hint,
                },
            ) if allow_incompatible => {
                let safe_name = Self::validate_safe_id(&file_name_hint, "download file name")
                    .map(|_| file_name_hint.as_str())
                    .unwrap_or("downloaded_package.txt");
                let dst = cfg.base_dir().join("downloads").join(safe_name);
                fs_utils::write_text_file(&dst, &raw)?;
                Ok(format!("Saved raw package file to {}", dst.display()))
            }
            _ => Err(MenuError::Config(format!(
                "Unsupported payload format for package kind {:?}",
                pkg.kind
            ))),
        }
    }

    fn install_patch_text(
        patcher: &mut PatchManager,
        pkg: &RemotePackageEntry,
        raw: &str,
        allow_incompatible: bool,
    ) -> Result<String, MenuError> {
        let mut parsed = PatchRepository::try_parse_patch_file_contents(raw)?;
        if parsed.is_empty() {
            return Err(MenuError::ParseError(String::from(
                "No valid patch manifest found in downloaded TOML",
            )));
        }
        if !allow_incompatible {
            for m in &parsed {
                if !m.is_fully_compatible() {
                    return Err(MenuError::Config(
                        m.incompatibility_reason().unwrap_or_else(|| {
                            String::from("Patch is incompatible with this Genubar version")
                        }),
                    ));
                }
            }
        }
        let mut ids = Vec::new();
        for mut manifest in parsed.drain(..) {
            let id = manifest.meta.id.clone();
            Self::validate_safe_id(&id, "patch id")?;
            manifest.meta.origin = PackageOrigin::DownloadedRemote {
                registry_url: pkg.source_url.clone(),
            };
            PatchRepository::save_manifest_to_dir(&patcher.packages_dir, &manifest)?;
            patcher.patches.retain(|p| p.meta.id != id);
            patcher.patches.push(manifest);
            ids.push(id);
        }
        Ok(format!("Saved patch(es): {}", ids.join(", ")))
    }

    fn install_icon_pack_files(
        cfg: &mut ConfigManager,
        pkg: &RemotePackageEntry,
        files: &HashMap<String, String>,
        allow_incompatible: bool,
    ) -> Result<String, MenuError> {
        let pack_raw = files.get("pack.toml").ok_or_else(|| {
            MenuError::NotFound(String::from("Icon pack archive missing pack.toml"))
        })?;
        let mut pack: IconPackManifest = toml::from_str(pack_raw)
            .map_err(|e| MenuError::ParseError(format!("Invalid pack.toml: {e}")))?;
        if !allow_incompatible && !pack.is_compatible() {
            return Err(MenuError::Config(format!(
                "Icon pack requires Genubar v{}",
                pack.min_app_version.as_deref().unwrap_or("newer")
            )));
        }
        if allow_incompatible && !pack.is_compatible() {
            pack.enabled = false;
        }
        let pack_id = if pack.id.trim().is_empty() {
            pkg.id.clone()
        } else {
            pack.id.clone()
        };
        Self::validate_safe_id(&pack_id, "icon pack id")?;
        let pack_dir = cfg.base_dir().join("icons").join(&pack_id);
        fs_utils::ensure_dir(&pack_dir)?;
        Self::write_safe_relative_files(&pack_dir, files)?;
        pack.pack_dir = pack_dir;
        let pack_name = pack.name.clone();
        cfg.icon_packs.retain(|p| p.id != pack_id);
        cfg.icon_packs.push(pack);
        cfg.icon_packs.sort_by_key(|p| p.priority);
        Ok(format!("Saved icon pack '{pack_name}'"))
    }

    pub fn search_packages(
        cfg: &ConfigManager,
        patcher: &PatchManager,
        remote_packages: &[RemotePackageEntry],
        query: &str,
        category: Option<PackageKind>,
    ) -> Vec<CatalogSearchItem> {
        search::search_packages(cfg, patcher, remote_packages, query, category)
    }

    pub fn is_package_installed(
        cfg: &ConfigManager,
        patcher: &PatchManager,
        pkg: &RemotePackageEntry,
    ) -> bool {
        match pkg.kind {
            PackageKind::Patch => {
                let in_mem = patcher.patches.iter().any(|p| p.meta.id == pkg.id);
                let on_disk = cfg
                    .base_dir()
                    .join("patches")
                    .join(format!("{}.toml", pkg.id))
                    .is_file();
                in_mem || on_disk
            }
            PackageKind::Profile => cfg.profiles.iter().any(|p| p.id == pkg.id),
            PackageKind::Style => {
                let css_name = format!("{}.css", pkg.id);
                cfg.styles
                    .list_styles()
                    .iter()
                    .any(|s| s.id == pkg.id || s.file_name == css_name)
            }
            PackageKind::IconPack => cfg.icon_packs.iter().any(|ic| ic.id == pkg.id),
            PackageKind::Bundle => BundleManager::new(cfg.base_dir().to_path_buf())
                .list_bundles()
                .iter()
                .any(|b| b.manifest.id == pkg.id),
            PackageKind::Script => cfg
                .base_dir()
                .join("scripts")
                .join(format!("{}.py", pkg.id))
                .is_file(),
            PackageKind::Unknown => ["toml", "txt", "json"].iter().any(|ext| {
                cfg.base_dir()
                    .join("downloads")
                    .join(format!("{}.{ext}", pkg.id))
                    .is_file()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genubar_patcher::PatchLedger;

    fn make_remote_entry(id: &str, kind: PackageKind, name: &str) -> RemotePackageEntry {
        RemotePackageEntry {
            id: id.to_string(),
            kind,
            name: name.to_string(),
            version: String::from("1.0.0"),
            min_app_version: None,
            author: String::from("tester"),
            description: String::from("Test package"),
            download_url: Some(String::from("https://example.invalid/pkg")),
            files: HashMap::new(),
            content: None,
            tags: vec![String::from("test")],
            source_url: String::from("https://example.invalid/registry.json"),
        }
    }

    #[test]
    fn installs_profile_style_icon_pack_and_patch_and_detects_installed_state() {
        let tmp_base = std::env::temp_dir().join("genubar_test_registry_installer");
        let _ = fs::remove_dir_all(&tmp_base);
        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let mut patcher = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: Vec::new(),
            ledger: PatchLedger::default(),
        };

        let profile_pkg = make_remote_entry("sample_app", PackageKind::Profile, "Sample App");
        let profile_toml = r#"
id = "sample_app"
name = "Sample App"
enabled = true

[matcher]
field = "AppId"
pattern = "org.example.Sample"
"#;
        let msg = RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &profile_pkg,
            FetchedPackagePayload::TextContent {
                raw: profile_toml.to_string(),
                file_name_hint: String::from("sample_app.toml"),
            },
            false,
        )
        .unwrap();
        assert!(msg.contains("Sample App"));
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg,
            &patcher,
            &profile_pkg
        ));

        let style_pkg = make_remote_entry("ocean_theme", PackageKind::Style, "Ocean Theme");
        let style_css = "/* ==GenubarStyle==\n * @name Ocean Theme\n * @version 1.0.0\n ==/GenubarStyle== */\nwindow { color: #fff; }\n";
        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &style_pkg,
            FetchedPackagePayload::TextContent {
                raw: style_css.to_string(),
                file_name_hint: String::from("ocean_theme.css"),
            },
            false,
        )
        .unwrap();
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg, &patcher, &style_pkg
        ));

        let icon_pkg = make_remote_entry("mono_icons", PackageKind::IconPack, "Mono Icons");
        let mut icon_files = HashMap::new();
        icon_files.insert(
            String::from("pack.toml"),
            String::from("id = \"mono_icons\"\nname = \"Mono Icons\"\npriority = 10\n"),
        );
        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &icon_pkg,
            FetchedPackagePayload::BundleFiles { files: icon_files },
            false,
        )
        .unwrap();
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg, &patcher, &icon_pkg
        ));

        let patch_pkg = make_remote_entry("demo_patch", PackageKind::Patch, "Demo Patch");
        let patch_toml = r#"
[meta]
id = "demo_patch"
name = "Demo Patch"
version = "1.0.0"
"#;
        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &patch_pkg,
            FetchedPackagePayload::TextContent {
                raw: patch_toml.to_string(),
                file_name_hint: String::from("demo_patch.toml"),
            },
            false,
        )
        .unwrap();
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg, &patcher, &patch_pkg
        ));
        assert!(tmp_base.join("patches/demo_patch.toml").is_file());

        let style_results = RegistryPackageInstaller::search_packages(
            &cfg,
            &patcher,
            &[profile_pkg.clone(), style_pkg.clone()],
            "ocean",
            Some(PackageKind::Style),
        );
        assert!(
            style_results
                .iter()
                .any(|item| item.kind == PackageKind::Style && item.is_local)
        );
        assert!(
            style_results
                .iter()
                .all(|item| item.kind == PackageKind::Style)
        );

        let _ = fs::remove_dir_all(&tmp_base);
    }

    #[test]
    fn blocks_unknown_kind_unless_allow_incompatible_saves_raw_download() {
        let tmp_base = std::env::temp_dir().join("genubar_test_registry_unknown_pkg");
        let _ = fs::remove_dir_all(&tmp_base);
        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let mut patcher = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: Vec::new(),
            ledger: PatchLedger::default(),
        };

        let unknown_pkg = make_remote_entry("future_widget", PackageKind::Unknown, "Future Widget");
        let err = RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &unknown_pkg,
            FetchedPackagePayload::TextContent {
                raw: String::from("{\"ok\":true}"),
                file_name_hint: String::from("future_widget.json"),
            },
            false,
        )
        .unwrap_err();
        assert!(matches!(err, MenuError::Config(_)));
        assert!(err.to_string().contains("Unknown"));

        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &unknown_pkg,
            FetchedPackagePayload::TextContent {
                raw: String::from("{\"ok\":true}"),
                file_name_hint: String::from("future_widget.json"),
            },
            true,
        )
        .unwrap();
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg,
            &patcher,
            &unknown_pkg
        ));

        let _ = fs::remove_dir_all(&tmp_base);
    }

    #[test]
    fn installs_script_rejects_traversal_and_persists_remote_patch_origin() {
        let tmp_base = std::env::temp_dir().join("genubar_test_registry_packet_b");
        let _ = fs::remove_dir_all(&tmp_base);
        let mut cfg = ConfigManager::load_from_base_dir(tmp_base.clone()).unwrap();
        let mut patcher = PatchManager {
            packages_dir: tmp_base.join("patches"),
            patches: Vec::new(),
            ledger: PatchLedger::default(),
        };

        // 1. Path traversal in package ID and archive relative paths must fail explicitly
        let bad_id_pkg = make_remote_entry("../escape", PackageKind::Script, "Evil");
        assert!(
            RegistryPackageInstaller::install_fetched_package_ext(
                &mut cfg,
                &mut patcher,
                &bad_id_pkg,
                FetchedPackagePayload::TextContent {
                    raw: String::from("# ==GenubarScript==\n# @id x\n# ==/GenubarScript=="),
                    file_name_hint: String::from("evil.py"),
                },
                false,
            )
            .is_err()
        );

        let traversal_icon = make_remote_entry("safe_pack", PackageKind::IconPack, "Safe Pack");
        let mut bad_files = HashMap::new();
        bad_files.insert(
            String::from("pack.toml"),
            String::from("id = \"safe_pack\"\nname = \"Safe Pack\"\n"),
        );
        bad_files.insert(String::from("../../.bashrc"), String::from("pwned"));
        assert!(
            RegistryPackageInstaller::install_fetched_package_ext(
                &mut cfg,
                &mut patcher,
                &traversal_icon,
                FetchedPackagePayload::BundleFiles { files: bad_files },
                false,
            )
            .is_err()
        );
        assert!(!tmp_base.join(".bashrc").exists());

        // 2. Script installation checks # ==GenubarScript== header and marks installed
        let script_pkg = make_remote_entry("sample_tx", PackageKind::Script, "Sample Transformer");
        assert!(
            RegistryPackageInstaller::install_fetched_package_ext(
                &mut cfg,
                &mut patcher,
                &script_pkg,
                FetchedPackagePayload::TextContent {
                    raw: String::from("print('no header')"),
                    file_name_hint: String::from("sample_tx.py"),
                },
                false,
            )
            .is_err()
        );

        let valid_script = "# ==GenubarScript==\n# @id sample_tx\n# @name Sample Transformer\n# ==/GenubarScript==\n";
        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &script_pkg,
            FetchedPackagePayload::TextContent {
                raw: valid_script.to_string(),
                file_name_hint: String::from("sample_tx.py"),
            },
            false,
        )
        .unwrap();
        assert!(RegistryPackageInstaller::is_package_installed(
            &cfg,
            &patcher,
            &script_pkg
        ));
        assert!(tmp_base.join("scripts/sample_tx.py").is_file());

        // 3. Single remote patch persists PackageOrigin::DownloadedRemote on disk reload
        let patch_pkg = make_remote_entry("remote_demo", PackageKind::Patch, "Remote Demo");
        let patch_toml = r#"
[meta]
id = "remote_demo"
name = "Remote Demo"
version = "1.0.0"
"#;
        RegistryPackageInstaller::install_fetched_package_ext(
            &mut cfg,
            &mut patcher,
            &patch_pkg,
            FetchedPackagePayload::TextContent {
                raw: patch_toml.to_string(),
                file_name_hint: String::from("remote_demo.toml"),
            },
            false,
        )
        .unwrap();
        let reloaded = PatchManager::load_from_packages_dir(tmp_base.join("patches")).unwrap();
        let remote_loaded = reloaded
            .patches
            .iter()
            .find(|p| p.meta.id == "remote_demo")
            .unwrap();
        assert_eq!(
            remote_loaded.meta.origin,
            PackageOrigin::DownloadedRemote {
                registry_url: String::from("https://example.invalid/registry.json"),
            }
        );

        let _ = fs::remove_dir_all(&tmp_base);
    }
}
