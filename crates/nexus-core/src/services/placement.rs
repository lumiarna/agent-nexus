use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use rusqlite::OptionalExtension;

use crate::{
    error::{AppError, AppResult},
    services::{
        paths::{path_to_string, resolve_local_path},
        symlink::{
            create_junction_placement, create_symlink_placement, is_junction, remove_symlink,
            remove_symlink_if_present,
        },
    },
};

#[derive(Debug, Clone)]
pub struct TaskLinkPlacement {
    action: String,
    target_type: String,
    target: String,
}

impl TaskLinkPlacement {
    pub fn remove_if_present(&self) -> AppResult<()> {
        if !task_owns_link_placement(&self.action, &self.target_type) {
            return Ok(());
        }

        remove_symlink_if_present(&resolve_local_path(&self.target)?)
    }
}

pub fn task_link_placement_for_task(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> AppResult<Option<TaskLinkPlacement>> {
    conn.query_row(
        r#"
        SELECT action, target_type, target
        FROM tasks
        WHERE id = ?1
        "#,
        [task_id],
        task_link_placement_from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn task_link_placements_for_group(
    conn: &rusqlite::Connection,
    group_id: &str,
) -> AppResult<Vec<TaskLinkPlacement>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT action, target_type, target
        FROM tasks
        WHERE group_id = ?1
        "#,
    )?;
    let rows = stmt.query_map([group_id], task_link_placement_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn task_managed_target_identities(conn: &rusqlite::Connection) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT action, target_type, target
        FROM tasks
        "#,
    )?;
    let rows = stmt.query_map([], task_link_placement_from_row)?;
    let mut identities = HashSet::new();

    for row in rows {
        let placement = row?;
        if !task_owns_link_placement(&placement.action, &placement.target_type) {
            continue;
        }

        let target = resolve_local_path(&placement.target)?;
        if link_entry_present(&target) {
            identities.insert(placement_entry_identity(&target)?);
        }
    }

    Ok(identities)
}

pub fn create_task_link_placement(
    action: &str,
    source: &str,
    target: &str,
) -> AppResult<Option<PathBuf>> {
    if !is_link_action(action) {
        return Ok(None);
    }

    let source = resolve_local_path(source)?;
    let target = resolve_local_path(target)?;
    let result = match action {
        "Symlink" => create_symlink_placement(&source, &target),
        "Junction" => create_junction_placement(&source, &target),
        _ => unreachable!("link action checked before placement creation"),
    };
    result.inspect_err(|_| {
        let _ = remove_symlink_if_present(&target);
    })?;
    Ok(Some(target))
}

pub fn remove_created_task_link_placements(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = remove_symlink_if_present(path);
    }
}

pub fn remove_unmanaged_link_placement(path: &Path) -> AppResult<()> {
    remove_symlink(path)
}

pub fn task_link_state(action: &str, target_type: &str, target: &str) -> &'static str {
    if task_owns_link_placement(action, target_type)
        && resolve_local_path(target)
            .map(|path| link_entry_present(&path))
            .unwrap_or(false)
    {
        "present"
    } else if task_owns_link_placement(action, target_type) {
        "missing"
    } else {
        "present"
    }
}

pub fn scanned_target_identity(path: &Path) -> AppResult<String> {
    placement_entry_identity(path)
}

pub fn is_link_action(action: &str) -> bool {
    matches!(action, "Symlink" | "Junction")
}

fn task_owns_link_placement(action: &str, target_type: &str) -> bool {
    is_link_action(action) && target_type == "Local"
}

fn task_link_placement_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskLinkPlacement> {
    Ok(TaskLinkPlacement {
        action: row.get(0)?,
        target_type: row.get(1)?,
        target: row.get(2)?,
    })
}

fn link_entry_present(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink() || is_junction(path))
        .unwrap_or(false)
}

fn placement_entry_identity(path: &Path) -> AppResult<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = absolute.parent().ok_or_else(|| {
        AppError::Validation(format!(
            "placement target must have a parent path: {}",
            absolute.display()
        ))
    })?;
    let name = absolute.file_name().ok_or_else(|| {
        AppError::Validation(format!(
            "placement target must include a final path segment: {}",
            absolute.display()
        ))
    })?;
    let parent = parent.canonicalize()?;

    path_to_string(&parent.join(name), "placement target path")
}
