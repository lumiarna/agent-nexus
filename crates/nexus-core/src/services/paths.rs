use std::{
    env,
    path::{Path, PathBuf},
};

use crate::error::{AppError, AppResult};

/// Return the native home directory used by every local path consumer.
///
/// On Windows, `USERPROFILE` is authoritative because Git Bash may expose a
/// POSIX-style `HOME` (for example, `/c/Users/name`) that native applications
/// cannot consume. `HOME` remains a fallback for Windows environments without
/// `USERPROFILE`. Other platforms use `HOME` only.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env_dir("USERPROFILE").or_else(|| env_dir("HOME"))
    }
    #[cfg(not(windows))]
    {
        env_dir("HOME")
    }
}

fn env_dir(variable: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn unresolved_home_error() -> AppError {
    #[cfg(windows)]
    let message = "cannot resolve '~': USERPROFILE and HOME environment variables are not set";
    #[cfg(not(windows))]
    let message = "cannot resolve '~': HOME environment variable is not set";

    AppError::Validation(message.to_string())
}

pub fn resolve_local_path(raw: &str) -> AppResult<PathBuf> {
    if raw == "~" {
        return home_dir().ok_or_else(unresolved_home_error);
    }
    let home_relative = raw.strip_prefix("~/").or_else(|| {
        #[cfg(windows)]
        {
            raw.strip_prefix(r"~\")
        }
        #[cfg(not(windows))]
        {
            None
        }
    });
    if let Some(rest) = home_relative {
        return home_dir()
            .map(|home| home.join(rest))
            .ok_or_else(unresolved_home_error);
    }
    if let Some(path) = expand_supported_env_path(raw)? {
        return Ok(path);
    }
    Ok(PathBuf::from(raw))
}

fn expand_supported_env_path(raw: &str) -> AppResult<Option<PathBuf>> {
    for variable in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(path) = expand_named_env_path(raw, variable)? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn expand_named_env_path(raw: &str, variable: &str) -> AppResult<Option<PathBuf>> {
    let token = format!("%{variable}%");
    if raw == token {
        return resolve_named_env_dir(variable).map(Some);
    }

    let Some(rest) = raw.strip_prefix(&token) else {
        return Ok(None);
    };
    let Some(rest) = rest.strip_prefix(['/', '\\']) else {
        return Ok(None);
    };
    Ok(Some(resolve_named_env_dir(variable)?.join(rest)))
}

fn resolve_named_env_dir(variable: &str) -> AppResult<PathBuf> {
    env::var_os(variable).map(PathBuf::from).ok_or_else(|| {
        AppError::Validation(format!(
            "cannot resolve '%{variable}%': {variable} environment variable is not set"
        ))
    })
}

pub fn path_to_string(path: &Path, label: &str) -> AppResult<String> {
    path.to_str()
        .map(normalize_display_path)
        .ok_or_else(|| AppError::Validation(format!("{label} must be valid UTF-8")))
}

pub fn path_buf_for_comparison(path: PathBuf, label: &str) -> AppResult<PathBuf> {
    Ok(PathBuf::from(path_to_string(&path, label)?))
}

/// Collapse a home-relative absolute path back to its `~` form for display. The
/// inverse of [`resolve_local_path`]'s tilde handling: paths are stored canonical
/// but shown with `~` so the UI never exposes a user's absolute home directory.
/// Anything not under the home directory (or a path that is already `~`-relative)
/// is returned unchanged.
pub fn collapse_home(path: &str) -> String {
    let Some(home) = home_dir().and_then(|home| home.to_str().map(ToOwned::to_owned)) else {
        return path.to_string();
    };
    if path == home {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(&home) {
        if rest.starts_with('/') || rest.starts_with('\\') {
            #[cfg(windows)]
            return format!("~/{}", rest[1..].replace('\\', "/"));
            #[cfg(not(windows))]
            return format!("~{rest}");
        }
    }
    path.to_string()
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

    fn with_home_vars<F: FnOnce()>(home: Option<&Path>, userprofile: Option<&Path>, f: F) {
        let previous_home = env::var_os("HOME");
        let previous_userprofile = env::var_os("USERPROFILE");
        match home {
            Some(path) => env::set_var("HOME", path),
            None => env::remove_var("HOME"),
        }
        match userprofile {
            Some(path) => env::set_var("USERPROFILE", path),
            None => env::remove_var("USERPROFILE"),
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

        match previous_home {
            Some(value) => env::set_var("HOME", value),
            None => env::remove_var("HOME"),
        }
        match previous_userprofile {
            Some(value) => env::set_var("USERPROFILE", value),
            None => env::remove_var("USERPROFILE"),
        }
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    fn with_home<F: FnOnce(&Path)>(home: &TempDir, f: F) {
        with_home_vars(Some(home.path()), Some(home.path()), || f(home.path()));
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
    #[serial]
    fn collapse_home_replaces_home_prefix_with_tilde() {
        let home = TempDir::new().expect("create temp home");
        with_home(&home, |home| {
            let home = home.to_str().expect("utf-8 home");
            assert_eq!(collapse_home(home), "~");
            assert_eq!(collapse_home(&format!("{home}/Vault")), "~/Vault");
            assert_eq!(
                collapse_home(&format!("{home}/Vault/Clipper")),
                "~/Vault/Clipper"
            );
        });
    }

    #[test]
    #[serial]
    fn collapse_home_passes_through_paths_outside_home() {
        let home = TempDir::new().expect("create temp home");
        with_home(&home, |home| {
            let home = home.to_str().expect("utf-8 home");
            // A sibling whose name merely starts with the home string must not match.
            assert_eq!(
                collapse_home(&format!("{home}x/foo")),
                format!("{home}x/foo")
            );
            assert_eq!(collapse_home("/opt/data"), "/opt/data");
            assert_eq!(collapse_home("~/already"), "~/already");
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
    #[serial]
    fn shared_home_resolver_prefers_userprofile_over_git_bash_home() {
        with_home_vars(
            Some(Path::new("/c/Users/SONGSH2")),
            Some(Path::new(r"C:\Users\SONGSH2")),
            || {
                assert_eq!(home_dir(), Some(PathBuf::from(r"C:\Users\SONGSH2")));
                assert_eq!(
                    resolve_local_path("~/.pi/agent").expect("resolve config root"),
                    PathBuf::from(r"C:\Users\SONGSH2\.pi\agent")
                );
                assert_eq!(
                    resolve_local_path(r"~\.pi\agent").expect("resolve collapsed Windows path"),
                    PathBuf::from(r"C:\Users\SONGSH2\.pi\agent")
                );
                assert_eq!(collapse_home(r"C:\Users\SONGSH2\.pi\agent"), "~/.pi/agent");
            },
        );
    }

    #[cfg(windows)]
    #[test]
    #[serial]
    fn shared_home_resolver_falls_back_to_home_without_userprofile() {
        with_home_vars(Some(Path::new(r"C:\FallbackHome")), None, || {
            assert_eq!(home_dir(), Some(PathBuf::from(r"C:\FallbackHome")));
            assert_eq!(
                resolve_local_path("~/config").expect("resolve fallback home"),
                PathBuf::from(r"C:\FallbackHome\config")
            );
        });
    }

    #[cfg(windows)]
    #[test]
    #[serial]
    fn resolve_local_path_expands_supported_windows_env_paths() {
        let root = TempDir::new().expect("create temp env root");
        let appdata = root.path().join("Roaming");
        let localappdata = root.path().join("Local");
        let previous_appdata = env::var_os("APPDATA");
        let previous_localappdata = env::var_os("LOCALAPPDATA");
        env::set_var("APPDATA", &appdata);
        env::set_var("LOCALAPPDATA", &localappdata);

        assert_eq!(
            resolve_local_path(r"%APPDATA%\Zed\settings.json").expect("resolve %APPDATA% path"),
            appdata.join("Zed").join("settings.json")
        );
        assert_eq!(
            resolve_local_path("%LOCALAPPDATA%/warp/Warp/config/settings.toml")
                .expect("resolve %LOCALAPPDATA% path"),
            localappdata
                .join("warp")
                .join("Warp")
                .join("config")
                .join("settings.toml")
        );

        match previous_appdata {
            Some(value) => env::set_var("APPDATA", value),
            None => env::remove_var("APPDATA"),
        }
        match previous_localappdata {
            Some(value) => env::set_var("LOCALAPPDATA", value),
            None => env::remove_var("LOCALAPPDATA"),
        }
    }

    #[cfg(windows)]
    #[test]
    #[serial]
    fn resolve_local_path_rejects_missing_supported_windows_env_path() {
        let previous_appdata = env::var_os("APPDATA");
        env::remove_var("APPDATA");

        let error = resolve_local_path("%APPDATA%/Zed/settings.json").expect_err("missing APPDATA");

        match previous_appdata {
            Some(value) => env::set_var("APPDATA", value),
            None => env::remove_var("APPDATA"),
        }
        assert!(error
            .to_string()
            .contains("APPDATA environment variable is not set"));
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
