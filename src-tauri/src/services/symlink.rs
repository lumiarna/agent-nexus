use std::{fs, path::Path};

use crate::error::{AppError, AppResult};

pub fn create_symlink_placement(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::Validation(format!(
            "symlink source does not exist: {}",
            source.display()
        )));
    }
    if fs::symlink_metadata(target).is_ok() {
        return Err(AppError::Validation(format!(
            "symlink target already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    create_symlink(source, target)
}

#[cfg(unix)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)?;
    } else {
        std::os::windows::fs::symlink_file(source, target)?;
    }
    Ok(())
}

pub fn remove_symlink(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_symlink() {
        return Err(AppError::Validation(
            "symlink target path must be a symlink".to_string(),
        ));
    }

    fs::remove_file(path)?;
    Ok(())
}

pub fn remove_symlink_if_present(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => remove_symlink(path),
        Ok(_) | Err(_) => Ok(()),
    }
}
