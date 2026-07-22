use std::{fs, io::ErrorKind, path::Path, process::Command};

use crate::error::{AppError, AppResult};

/// Open `path` with the OS default handler (file in its app, directory in the file manager).
///
/// The target must exist so native file managers cannot silently replace an invalid target with
/// their default landing page.
pub fn open_path(path: &Path) -> AppResult<()> {
    ensure_path_exists(path)?;
    open_path_with_system(path)
}

fn ensure_path_exists(path: &Path) -> AppResult<()> {
    match fs::metadata(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(AppError::Validation(format!(
            "path does not exist: {}",
            path.display()
        ))),
        Err(error) => Err(AppError::Io(format!(
            "failed to inspect path {}: {error}",
            path.display()
        ))),
    }
}

#[cfg(target_os = "macos")]
fn open_path_with_system(path: &Path) -> AppResult<()> {
    Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_path_with_system(path: &Path) -> AppResult<()> {
    Command::new("explorer").arg(path).spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path_with_system(path: &Path) -> AppResult<()> {
    Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

/// Reveal `path` in the OS file manager, selecting the entry where the platform supports it.
///
/// As with [`open_path`], the target must exist before a native file manager is launched.
pub fn reveal_path(path: &Path) -> AppResult<()> {
    ensure_path_exists(path)?;
    reveal_path_with_system(path)
}

#[cfg(target_os = "macos")]
fn reveal_path_with_system(path: &Path) -> AppResult<()> {
    Command::new("open").arg("-R").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal_path_with_system(path: &Path) -> AppResult<()> {
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_path_with_system(path: &Path) -> AppResult<()> {
    let target = path.parent().unwrap_or(path);
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing_target() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("create temp dir");
        let missing = root.path().join("missing");
        (root, missing)
    }

    fn assert_missing_target_error(error: AppError) {
        assert!(error.to_string().contains("path does not exist"));
        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn open_path_rejects_missing_target_before_launching_system_handler() {
        let (_root, missing) = missing_target();

        let error = open_path(&missing).expect_err("missing target must fail");

        assert_missing_target_error(error);
    }

    #[test]
    fn reveal_path_rejects_missing_target_before_launching_system_handler() {
        let (_root, missing) = missing_target();

        let error = reveal_path(&missing).expect_err("missing target must fail");

        assert_missing_target_error(error);
    }
}
