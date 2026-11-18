use genubar_core::MenuError;
use std::fs;
use std::path::Path;

pub fn ensure_dir(dir: impl AsRef<Path>) -> Result<(), MenuError> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .map_err(|e| MenuError::IoPermission(format!("Failed creating {}: {e}", dir.display())))
}

pub fn ensure_parent_dir(file_path: impl AsRef<Path>) -> Result<(), MenuError> {
    let file_path = file_path.as_ref();
    if let Some(parent) = file_path.parent()
        && !parent.as_os_str().is_empty() {
            ensure_dir(parent)?;
        }
    Ok(())
}

pub fn read_text_file(path: impl AsRef<Path>) -> Result<String, MenuError> {
    let path = path.as_ref();
    fs::read_to_string(path)
        .map_err(|e| MenuError::IoPermission(format!("Failed reading {}: {e}", path.display())))
}

pub fn write_text_file(path: impl AsRef<Path>, content: &str) -> Result<(), MenuError> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    fs::write(path, content)
        .map_err(|e| MenuError::IoPermission(format!("Failed writing {}: {e}", path.display())))
}

pub fn write_bytes_file(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), MenuError> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    fs::write(path, bytes)
        .map_err(|e| MenuError::IoPermission(format!("Failed writing {}: {e}", path.display())))
}

pub fn set_executable_permissions(path: impl AsRef<Path>) -> Result<(), MenuError> {
    let path = path.as_ref();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| MenuError::IoPermission(format!("Stat {}: {e}", path.display())))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| MenuError::IoPermission(format!("Chmod {}: {e}", path.display())))?;
    }
    Ok(())
}

pub fn write_executable_file(path: impl AsRef<Path>, content: &str) -> Result<(), MenuError> {
    let path = path.as_ref();
    write_text_file(path, content)?;
    set_executable_permissions(path)
}

pub fn copy_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), MenuError> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    ensure_parent_dir(dst)?;
    fs::copy(src, dst).map_err(|e| {
        MenuError::IoPermission(format!("Copy {} -> {}: {e}", src.display(), dst.display()))
    })?;
    Ok(())
}

pub fn copy_executable_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), MenuError> {
    let dst = dst.as_ref();
    copy_file(src, dst)?;
    set_executable_permissions(dst)
}

pub fn copy_dir_recursive(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), MenuError> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    ensure_dir(dst)?;
    let entries = fs::read_dir(src)
        .map_err(|e| MenuError::IoPermission(format!("Read dir {}: {e}", src.display())))?;
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            copy_file(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

pub fn replace_with_atomic_copy_or_symlink(
    target: impl AsRef<Path>,
    link_path: impl AsRef<Path>,
) -> Result<(), MenuError> {
    let target = target.as_ref();
    let link_path = link_path.as_ref();
    ensure_parent_dir(link_path)?;
    if let Ok(meta) = fs::symlink_metadata(link_path)
        && (meta.file_type().is_symlink() || meta.is_file()) {
            let _ = fs::remove_file(link_path);
        }

    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(target, link_path).is_ok() {
            return Ok(());
        }
    }

    copy_file(target, link_path)
}

pub fn remove_file_if_exists(path: impl AsRef<Path>) -> Result<(), MenuError> {
    let path = path.as_ref();
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path)
            .map_err(|e| MenuError::IoPermission(format!("Remove {}: {e}", path.display())))?;
    }
    Ok(())
}

pub fn remove_dir_if_exists(path: impl AsRef<Path>) -> Result<(), MenuError> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|e| MenuError::IoPermission(format!("Remove dir {}: {e}", path.display())))?;
    }
    Ok(())
}
