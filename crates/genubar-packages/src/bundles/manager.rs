use genubar_config::{BundleManifest, BundleStatusView};
use genubar_core::MenuError;
use genubar_platform::fs_utils::{
    copy_dir_recursive, copy_file, ensure_dir, read_text_file, remove_dir_if_exists,
    remove_file_if_exists,
};
use std::fs;
use std::path::{Path, PathBuf};

pub struct BundleManager {
    base_dir: PathBuf,
    bundles_dir: PathBuf,
}

impl BundleManager {
    pub fn new(base_dir: PathBuf) -> Self {
        let bundles_dir = base_dir.join("bundles");
        if let Err(err) = ensure_dir(&bundles_dir) {
            tracing::warn!(path = %bundles_dir.display(), error = %err, "Failed creating bundles directory");
        }
        Self {
            base_dir,
            bundles_dir,
        }
    }

    pub fn list_bundles(&self) -> Vec<BundleStatusView> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.bundles_dir) else {
            return out;
        };

        for entry in entries.flatten() {
            let manifest_path = entry.path().join("bundle.toml");
            if manifest_path.is_file() {
                match Self::read_manifest(&manifest_path) {
                    Ok(manifest) => {
                        let (installed, total) = self.count_installed_components(&manifest);
                        let is_compatible = manifest.is_compatible();
                        out.push(BundleStatusView {
                            manifest,
                            total_components: total,
                            installed_components: installed,
                            is_compatible,
                        });
                    }
                    Err(err) => tracing::warn!(error = %err, "Failed loading bundle manifest"),
                }
            }
        }

        out
    }

    fn read_manifest(path: &Path) -> Result<BundleManifest, MenuError> {
        let raw = read_text_file(path)?;
        toml::from_str::<BundleManifest>(&raw)
            .map_err(|e| MenuError::Config(format!("TOML parse error in {}: {e}", path.display())))
    }

    pub fn count_installed_components(&self, m: &BundleManifest) -> (usize, usize) {
        let total = m.patches.len() + m.profiles.len() + m.styles.len() + m.icon_packs.len();
        let mut installed = 0;

        for p in &m.patches {
            if self.base_dir.join("patches").join(p).exists() {
                installed += 1;
            }
        }
        for pr in &m.profiles {
            if self.base_dir.join("profiles").join(pr).exists() {
                installed += 1;
            }
        }
        for st in &m.styles {
            if self.base_dir.join("styles").join(st).exists() {
                installed += 1;
            }
        }
        for ic in &m.icon_packs {
            if self
                .base_dir
                .join("icons")
                .join(ic)
                .join("pack.toml")
                .exists()
            {
                installed += 1;
            }
        }

        (installed, total)
    }

    pub fn install_bundle_from_dir(&self, source_dir: &Path) -> Result<String, MenuError> {
        self.install_bundle_from_dir_ext(source_dir, false)
    }

    pub fn install_bundle_from_dir_ext(
        &self,
        source_dir: &Path,
        allow_incompatible: bool,
    ) -> Result<String, MenuError> {
        let manifest_file = source_dir.join("bundle.toml");
        let manifest: BundleManifest = Self::read_manifest(&manifest_file)?;
        if !allow_incompatible && !manifest.is_compatible() {
            return Err(MenuError::Config(format!(
                "Bundle '{}' requires Genubar v{} (update Genubar first)",
                manifest.name,
                manifest.min_app_version.as_deref().unwrap_or("newer")
            )));
        }

        for (subdir, file_list) in [
            ("patches", &manifest.patches),
            ("profiles", &manifest.profiles),
            ("styles", &manifest.styles),
        ] {
            let dst_dir = self.base_dir.join(subdir);
            ensure_dir(&dst_dir)?;
            for fname in file_list {
                let src_f = source_dir.join(subdir).join(fname);
                if src_f.is_file() {
                    copy_file(&src_f, dst_dir.join(fname))?;
                }
            }
        }

        for pack_name in &manifest.icon_packs {
            let src_pack = source_dir.join("icons").join(pack_name);
            let dst_pack = self.base_dir.join("icons").join(pack_name);
            if src_pack.is_dir() {
                copy_dir_recursive(&src_pack, &dst_pack)?;
            }
        }

        let bundle_record_dir = self.bundles_dir.join(&manifest.id);
        ensure_dir(&bundle_record_dir)?;
        copy_file(&manifest_file, bundle_record_dir.join("bundle.toml"))?;

        Ok(manifest.id)
    }

    pub fn uninstall_bundle(&self, bundle_id: &str) -> Result<(), MenuError> {
        let record_dir = self.bundles_dir.join(bundle_id);
        let manifest_path = record_dir.join("bundle.toml");
        if manifest_path.is_file()
            && let Ok(manifest) = Self::read_manifest(&manifest_path)
        {
            for p in &manifest.patches {
                let target = self.base_dir.join("patches").join(p);
                if let Err(err) = remove_file_if_exists(&target) {
                    tracing::warn!(path = %target.display(), error = %err, "Failed removing bundle patch");
                }
            }
            for pr in &manifest.profiles {
                let target = self.base_dir.join("profiles").join(pr);
                if let Err(err) = remove_file_if_exists(&target) {
                    tracing::warn!(path = %target.display(), error = %err, "Failed removing bundle profile");
                }
            }
            for st in &manifest.styles {
                let target = self.base_dir.join("styles").join(st);
                if let Err(err) = remove_file_if_exists(&target) {
                    tracing::warn!(path = %target.display(), error = %err, "Failed removing bundle style");
                }
            }
            for ic in &manifest.icon_packs {
                let target = self.base_dir.join("icons").join(ic);
                if let Err(err) = remove_dir_if_exists(&target) {
                    tracing::warn!(path = %target.display(), error = %err, "Failed removing bundle icon pack");
                }
            }
        }
        remove_dir_if_exists(&record_dir)
    }
}
