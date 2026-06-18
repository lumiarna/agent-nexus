use std::{fs, io, path::Path};

use crate::error::{AppError, AppResult};

/// Shared pre-flight checks before placing a link entry at `target`.
fn ensure_placeable(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::Validation(format!(
            "link source does not exist: {}",
            source.display()
        )));
    }
    if fs::symlink_metadata(target).is_ok() {
        return Err(AppError::Validation(format!(
            "link target already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn create_symlink_placement(source: &Path, target: &Path) -> AppResult<()> {
    ensure_placeable(source, target)?;
    create_symlink(source, target)
}

#[cfg(unix)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    let result = if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)
    } else {
        std::os::windows::fs::symlink_file(source, target)
    };
    result.map_err(|error| map_create_symlink_error(error, source, target))
}

#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

/// Turn the "symlink privilege not held" failure into actionable guidance: either
/// switch the task to a Junction, or create the link manually with `mklink` from an
/// elevated prompt. Other errors pass through unchanged.
#[cfg(windows)]
fn map_create_symlink_error(error: io::Error, source: &Path, target: &Path) -> AppError {
    if error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) {
        let flag = if source.is_dir() { " /D" } else { "" };
        AppError::Validation(format!(
            "Creating a symbolic link requires elevated privileges (run as Administrator, or enable Windows Developer Mode).\n\
             You can either:\n\
             1) Switch this task's action to Junction — it links directories without elevation, or\n\
             2) Create the link yourself from an elevated Command Prompt (Run as administrator):\n\
             mklink{flag} \"{target}\" \"{source}\"",
            target = target.display(),
            source = source.display(),
        ))
    } else {
        AppError::from(error)
    }
}

/// Place a directory junction at `target` pointing to `source`. Windows-only; junctions
/// require no special privilege but only work for directories.
#[cfg(windows)]
pub fn create_junction_placement(source: &Path, target: &Path) -> AppResult<()> {
    if !source.is_dir() {
        return Err(AppError::Validation(format!(
            "junction source must be a directory: {}",
            source.display()
        )));
    }
    ensure_placeable(source, target)?;
    junction::create(source, target).map_err(|error| {
        let _ = fs::remove_dir(target);
        AppError::from(error)
    })
}

#[cfg(not(windows))]
pub fn create_junction_placement(_source: &Path, _target: &Path) -> AppResult<()> {
    Err(AppError::Validation(
        "Junction links are only supported on Windows".to_string(),
    ))
}

/// Place a directory link for managed asset distribution (skills/prompts), where the user has no
/// explicit action choice. Prefers a symlink; on Windows lacking symlink privilege it falls back
/// to a junction so distribution still works without elevation. Unix always uses a symlink.
#[cfg(windows)]
pub fn create_managed_directory_link(source: &Path, target: &Path) -> AppResult<()> {
    ensure_placeable(source, target)?;
    if !source.is_dir() {
        return std::os::windows::fs::symlink_file(source, target)
            .map_err(|error| map_create_symlink_error(error, source, target));
    }
    match std::os::windows::fs::symlink_dir(source, target) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) => {
            junction::create(source, target).map_err(|error| {
                let _ = fs::remove_dir(target);
                AppError::from(error)
            })
        }
        Err(error) => Err(map_create_symlink_error(error, source, target)),
    }
}

#[cfg(not(windows))]
pub fn create_managed_directory_link(source: &Path, target: &Path) -> AppResult<()> {
    create_symlink_placement(source, target)
}

pub fn remove_symlink(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_symlink() && !is_junction(path) {
        return Err(AppError::Validation(
            "target path is not a symlink or junction".to_string(),
        ));
    }

    remove_link_entry(path)?;
    Ok(())
}

#[cfg(windows)]
pub fn is_junction(path: &Path) -> bool {
    junction::exists(path).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_junction(_path: &Path) -> bool {
    false
}

#[cfg(unix)]
fn remove_link_entry(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_link_entry(path: &Path) -> io::Result<()> {
    if is_junction(path) {
        // RemoveDirectory drops the junction entry itself without recursing into
        // (or deleting) its target contents.
        return fs::remove_dir(path);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            fs::remove_dir(path).map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

pub fn remove_symlink_if_present(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_junction(path) => {
            remove_symlink(path)
        }
        Ok(_) | Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn creates_symlink_placement_and_removes_link_only() {
        let root = tempfile::TempDir::new().expect("create temp dir");
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).expect("create source dir");
        fs::write(source.join("SKILL.md"), "# Test\n").expect("write source file");

        create_symlink_placement(&source, &target).expect("create link placement");

        assert!(fs::symlink_metadata(&target)
            .expect("read target metadata")
            .file_type()
            .is_symlink());

        remove_symlink(&target).expect("remove link placement");

        assert!(!target.exists());
        assert!(source.exists());
        assert!(source.join("SKILL.md").exists());
    }

    #[cfg(windows)]
    #[test]
    fn creates_junction_placement_and_removes_link_only() {
        let root = tempfile::TempDir::new().expect("create temp dir");
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).expect("create source dir");
        fs::write(source.join("SKILL.md"), "# Test\n").expect("write source file");

        create_junction_placement(&source, &target).expect("create junction placement");

        assert!(is_junction(&target));
        assert!(target.join("SKILL.md").exists());

        remove_symlink(&target).expect("remove junction placement");

        assert!(!target.exists());
        assert!(source.exists());
        assert!(source.join("SKILL.md").exists());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_junction_for_file_source() {
        let root = tempfile::TempDir::new().expect("create temp dir");
        let source = root.path().join("source.txt");
        let target = root.path().join("target");
        fs::write(&source, "x").expect("write source file");

        let error =
            create_junction_placement(&source, &target).expect_err("file source must be rejected");

        assert!(error.to_string().contains("must be a directory"));
        assert!(!target.exists());
    }
}
