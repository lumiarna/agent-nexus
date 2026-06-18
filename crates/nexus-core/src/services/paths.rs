use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

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
    use std::path::Path;

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
