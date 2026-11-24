use crate::error::InstallerError;
use genubar_core::CURRENT_APP_VERSION;
use genubar_platform::{cache_dir, check_embedded_extension_compatibility, fs_utils};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn host_platform_tag() -> String {
    format!("linux-{}", env::consts::ARCH)
}

pub fn release_archive_filename() -> String {
    format!(
        "genubar-v{CURRENT_APP_VERSION}-{}.tar.gz",
        host_platform_tag()
    )
}

pub fn cache_dir_for_version() -> PathBuf {
    cache_dir()
        .join("installer")
        .join(format!("v{CURRENT_APP_VERSION}"))
}

pub fn release_archive_url() -> String {
    format!(
        "https://github.com/genubar/genubar/releases/download/v{CURRENT_APP_VERSION}/{}",
        release_archive_filename()
    )
}

pub fn check_bundled_extension_compatibility() -> Result<String, InstallerError> {
    check_embedded_extension_compatibility().map_err(|e| InstallerError::Extension(e.to_string()))
}

fn find_file_recursive(root: &Path, target_name: &str, depth: usize) -> Option<PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    let direct = root.join(target_name);
    if direct.is_file() {
        return Some(direct);
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir()
            && let Some(found) = find_file_recursive(&p, target_name, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn build_local_search_dirs(
    explicit_target: Option<&str>,
    exe_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(trimmed) = explicit_target.map(str::trim).filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(trimmed));
    }

    if let Some(dir) = exe_dir {
        dirs.push(dir);
    }

    #[cfg(debug_assertions)]
    {
        dirs.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/release"
        )));
        dirs.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/debug"
        )));
    }

    dirs.extend([
        PathBuf::from("target/release"),
        PathBuf::from("genubar/target/release"),
        PathBuf::from("target/debug"),
        PathBuf::from("genubar/target/debug"),
        PathBuf::from("."),
    ]);
    dirs
}

fn local_search_dirs() -> Vec<PathBuf> {
    let explicit = env::var("GENUBAR_TARGET_DIR").ok();
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    build_local_search_dirs(explicit.as_deref(), exe_dir)
}

pub fn search_local_artifact(names: &[&str]) -> Option<PathBuf> {
    let cache_dir = cache_dir_for_version();
    let dirs = local_search_dirs();

    for name in names {
        for d in &dirs {
            let cand = d.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
        if let Some(cached) = find_file_recursive(&cache_dir, name, 3) {
            return Some(cached);
        }
    }
    None
}

/// Downloads and caches the matching GitHub Release tarball into `~/.cache/genubar/installer/v<ver>/`.
pub fn download_release_archive_to_cache() -> Result<PathBuf, InstallerError> {
    let cache_dir = cache_dir_for_version();
    fs::create_dir_all(&cache_dir).map_err(|source| InstallerError::Io {
        context: format!("create cache dir {}", cache_dir.display()),
        source,
    })?;

    let tgz_path = cache_dir.join(release_archive_filename());
    let url = release_archive_url();

    let dl = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", "-o"])
        .arg(&tgz_path)
        .arg(&url)
        .output()
        .map_err(|e| InstallerError::CommandFailed {
            command: String::from("curl"),
            details: e.to_string(),
        })?;

    if !dl.status.success() {
        return Err(InstallerError::CommandFailed {
            command: format!("curl {url}"),
            details: String::from_utf8_lossy(&dl.stderr).trim().to_string(),
        });
    }

    let untar = Command::new("tar")
        .arg("-xzf")
        .arg(&tgz_path)
        .arg("-C")
        .arg(&cache_dir)
        .output()
        .map_err(|e| InstallerError::CommandFailed {
            command: String::from("tar"),
            details: e.to_string(),
        })?;

    if !untar.status.success() {
        return Err(InstallerError::CommandFailed {
            command: String::from("tar -xzf"),
            details: String::from_utf8_lossy(&untar.stderr).trim().to_string(),
        });
    }

    Ok(cache_dir)
}

pub fn resolve_or_fetch_artifact(names: &[&str]) -> Result<PathBuf, InstallerError> {
    if let Some(found) = search_local_artifact(names) {
        return Ok(found);
    }
    download_release_archive_to_cache()?;
    search_local_artifact(names).ok_or_else(|| InstallerError::ArtifactNotFound {
        artifacts: names.iter().map(|s| (*s).to_string()).collect(),
        version: CURRENT_APP_VERSION.to_string(),
    })
}

pub fn copy_executable(src: &Path, dst: &Path) -> Result<(), InstallerError> {
    fs_utils::copy_executable_file(src, dst).map_err(|e| InstallerError::Io {
        context: format!("copy executable {} -> {}", src.display(), dst.display()),
        source: std::io::Error::other(e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_paths_and_embedded_extension_are_valid() {
        check_bundled_extension_compatibility().unwrap();
        let platform = host_platform_tag();
        assert!(platform.starts_with("linux-"));
        assert!(release_archive_filename().contains(&platform));
        assert!(release_archive_url().ends_with(&release_archive_filename()));
    }

    #[test]
    fn local_search_dirs_prioritize_explicit_override_then_exe_dir_before_dev_targets() {
        let explicit = PathBuf::from("/opt/genubar-override");
        let exe_dir = PathBuf::from("/tmp/extracted-release");
        let dirs = build_local_search_dirs(Some(explicit.to_str().unwrap()), Some(exe_dir.clone()));

        assert_eq!(dirs[0], explicit);
        assert_eq!(dirs[1], exe_dir);

        let release_idx = dirs
            .iter()
            .position(|d| d == &PathBuf::from("target/release"))
            .unwrap();
        let debug_idx = dirs
            .iter()
            .position(|d| d == &PathBuf::from("target/debug"))
            .unwrap();
        assert!(1 < release_idx && release_idx < debug_idx);
    }
}
