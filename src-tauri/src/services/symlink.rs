use std::{fs, io, path::Path};

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
        create_directory_link(source, target)?;
    } else {
        std::os::windows::fs::symlink_file(source, target)?;
    }
    Ok(())
}

#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

#[cfg(windows)]
fn create_directory_link(source: &Path, target: &Path) -> io::Result<()> {
    create_directory_link_with(
        source,
        target,
        |source, target| std::os::windows::fs::symlink_dir(source, target),
        |source, target| create_junction(source, target),
    )
}

#[cfg(windows)]
fn create_directory_link_with<S, J>(
    source: &Path,
    target: &Path,
    create_symlink: S,
    mut create_junction: J,
) -> io::Result<()>
where
    S: FnOnce(&Path, &Path) -> io::Result<()>,
    J: FnMut(&Path, &Path) -> io::Result<()>,
{
    match create_symlink(source, target) {
        Ok(()) => Ok(()),
        Err(error) if is_symlink_privilege_error(&error) => create_junction(source, target),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn is_symlink_privilege_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD)
}

#[cfg(windows)]
fn create_junction(source: &Path, target: &Path) -> io::Result<()> {
    let result = junction::create(source, target);
    if result.is_err() {
        let _ = fs::remove_dir(target);
    }
    result
}

pub fn remove_symlink(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_symlink() {
        return Err(AppError::Validation(
            "symlink target path must be a symlink".to_string(),
        ));
    }

    remove_link_entry(path)?;
    Ok(())
}

#[cfg(unix)]
fn remove_link_entry(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_link_entry(path: &Path) -> io::Result<()> {
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
        Ok(metadata) if metadata.file_type().is_symlink() => remove_symlink(path),
        Ok(_) | Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_link_points_to(source: &Path, target: &Path) {
        let raw_link = fs::read_link(target).expect("read target link");
        let resolved = if raw_link.is_absolute() {
            raw_link
        } else {
            target
                .parent()
                .map(|parent| parent.join(&raw_link))
                .unwrap_or(raw_link)
        };

        assert_eq!(
            fs::canonicalize(resolved).expect("canonicalize resolved link"),
            fs::canonicalize(source).expect("canonicalize source")
        );
    }

    #[test]
    fn creates_directory_link_placement_and_removes_link_only() {
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
        assert_link_points_to(&source, &target);

        remove_symlink(&target).expect("remove link placement");

        assert!(!target.exists());
        assert!(source.exists());
        assert!(source.join("SKILL.md").exists());
    }

    #[cfg(windows)]
    #[test]
    fn falls_back_to_junction_on_missing_symlink_privilege() {
        let root = tempfile::TempDir::new().expect("create temp dir");
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).expect("create source dir");
        let mut used_junction = false;

        create_directory_link_with(
            &source,
            &target,
            |_source, _target| Err(io::Error::from_raw_os_error(ERROR_PRIVILEGE_NOT_HELD)),
            |_source, _target| {
                used_junction = true;
                Ok(())
            },
        )
        .expect("fallback to junction");

        assert!(used_junction);
    }

    #[cfg(windows)]
    #[test]
    fn does_not_fall_back_to_junction_for_other_symlink_errors() {
        let root = tempfile::TempDir::new().expect("create temp dir");
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).expect("create source dir");
        let mut used_junction = false;

        let error = create_directory_link_with(
            &source,
            &target,
            |_source, _target| Err(io::Error::from_raw_os_error(5)),
            |_source, _target| {
                used_junction = true;
                Ok(())
            },
        )
        .expect_err("propagate non-privilege error");

        assert_eq!(error.raw_os_error(), Some(5));
        assert!(!used_junction);
    }
}
