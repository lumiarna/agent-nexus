use std::{
    env,
    path::{Path, PathBuf},
};

use crate::error::{AppError, AppResult};

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn resolve_local_path(raw: &str) -> AppResult<PathBuf> {
    if raw == "~" {
        return home_dir().ok_or_else(|| {
            AppError::Validation(
                "cannot resolve '~': HOME environment variable is not set".to_string(),
            )
        });
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir().map(|home| home.join(rest)).ok_or_else(|| {
            AppError::Validation(
                "cannot resolve '~': HOME environment variable is not set".to_string(),
            )
        });
    }
    Ok(PathBuf::from(raw))
}

pub fn path_to_string(path: &Path, label: &str) -> AppResult<String> {
    path.to_str()
        .map(normalize_display_path)
        .ok_or_else(|| AppError::Validation(format!("{label} must be valid UTF-8")))
}

pub fn path_buf_for_comparison(path: PathBuf, label: &str) -> AppResult<PathBuf> {
    Ok(PathBuf::from(path_to_string(&path, label)?))
}

#[cfg(windows)]
fn normalize_display_path(path: &str) -> String {
    if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", path);
    }

    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

#[cfg(not(windows))]
fn normalize_display_path(path: &str) -> String {
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::{env, path::Path};
    use tempfile::TempDir;

    fn with_home<F: FnOnce(&Path)>(home: &TempDir, f: F) {
        let previous = env::var_os("HOME");
        env::set_var("HOME", home.path());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(home.path())));
        match previous {
            Some(value) => env::set_var("HOME", value),
            None => env::remove_var("HOME"),
        }
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    #[serial]
    fn resolve_local_path_expands_tilde_forms_to_home() {
        let home = TempDir::new().expect("create temp home");
        with_home(&home, |home| {
            assert_eq!(
                resolve_local_path("~/foo").expect("resolve ~/foo"),
                home.join("foo")
            );
            assert_eq!(resolve_local_path("~").expect("resolve ~"), home);
        });
    }

    #[test]
    fn resolve_local_path_passes_through_non_tilde_paths() {
        assert_eq!(
            resolve_local_path("/abs/path").expect("resolve /abs/path"),
            PathBuf::from("/abs/path")
        );
        assert_eq!(
            resolve_local_path("rel/path").expect("resolve rel/path"),
            PathBuf::from("rel/path")
        );
        assert_eq!(
            resolve_local_path("plain").expect("resolve plain"),
            PathBuf::from("plain")
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_windows_verbatim_drive_prefix() {
        assert_eq!(
            path_to_string(Path::new(r"\\?\D:\Workspace\agent-nexus"), "path").unwrap(),
            r"D:\Workspace\agent-nexus"
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_windows_verbatim_unc_prefix() {
        assert_eq!(
            path_to_string(Path::new(r"\\?\UNC\server\share\repo"), "path").unwrap(),
            r"\\server\share\repo"
        );
    }
}
