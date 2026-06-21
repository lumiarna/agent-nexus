use std::{path::Path, process::Command};

use crate::error::AppResult;

/// Open `path` with the OS default handler (file in its app, directory in the file manager).
#[cfg(target_os = "macos")]
pub fn open_path(path: &Path) -> AppResult<()> {
    Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn open_path(path: &Path) -> AppResult<()> {
    Command::new("explorer").arg(path).spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn open_path(path: &Path) -> AppResult<()> {
    Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

/// Reveal `path` in the OS file manager, selecting the entry where the platform supports it.
#[cfg(target_os = "macos")]
pub fn reveal_path(path: &Path) -> AppResult<()> {
    Command::new("open").arg("-R").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn reveal_path(path: &Path) -> AppResult<()> {
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn reveal_path(path: &Path) -> AppResult<()> {
    let target = path.parent().unwrap_or(path);
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}
