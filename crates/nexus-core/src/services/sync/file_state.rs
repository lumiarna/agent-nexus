use std::{collections::HashMap, fs, path::Path, time::UNIX_EPOCH};

use rusqlite::params;

use crate::{
    error::{AppError, AppResult},
    services::util::now_epoch_seconds,
};

pub(super) type FileStateMap = HashMap<String, (u64, i64)>;

pub(super) struct FileState;

impl FileState {
    pub(super) fn load(conn: &rusqlite::Connection, task_id: &str) -> AppResult<FileStateMap> {
        let mut stmt = conn.prepare(
            "SELECT rel_path, file_size, file_mtime FROM task_file_state WHERE task_id = ?1",
        )?;
        let rows = stmt.query_map([task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, u64>(1)?, row.get::<_, i64>(2)?),
            ))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            map.insert(key, value);
        }
        Ok(map)
    }

    pub(super) fn record(
        conn: &rusqlite::Connection,
        task_id: &str,
        source: &Path,
    ) -> AppResult<()> {
        if source.is_dir() {
            Self::refresh_directory(conn, task_id, source)
        } else if source.is_file() {
            Self::save_single_file(conn, task_id, source)
        } else {
            Ok(())
        }
    }

    pub(super) fn should_skip(
        source: &Path,
        rel_path: &str,
        file_states: &FileStateMap,
    ) -> AppResult<bool> {
        if let Some(&(stored_size, stored_mtime)) = file_states.get(rel_path) {
            let metadata = fs::metadata(source)?;
            let current_size = metadata.len();
            let current_mtime = file_mtime_epoch(source)?;
            if stored_size == current_size && stored_mtime == current_mtime {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn refresh_directory(
        conn: &rusqlite::Connection,
        task_id: &str,
        source_root: &Path,
    ) -> AppResult<()> {
        conn.execute("DELETE FROM task_file_state WHERE task_id = ?1", [task_id])?;
        if source_root.is_dir() {
            let now = now_epoch_seconds()?;
            insert_recursive(conn, task_id, source_root, source_root, now)?;
        }
        Ok(())
    }

    fn save_single_file(
        conn: &rusqlite::Connection,
        task_id: &str,
        source: &Path,
    ) -> AppResult<()> {
        let rel_path = required_file_name(source)?;
        let metadata = fs::metadata(source)?;
        let size = metadata.len() as i64;
        let mtime = file_mtime_epoch(source)?;
        let now = now_epoch_seconds()?;
        conn.execute(
            "INSERT INTO task_file_state (task_id, rel_path, file_size, file_mtime, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(task_id, rel_path) DO UPDATE SET
                 file_size = excluded.file_size,
                 file_mtime = excluded.file_mtime,
                 updated_at = excluded.updated_at",
            params![task_id, rel_path, size, mtime, now],
        )?;
        Ok(())
    }
}

fn insert_recursive(
    conn: &rusqlite::Connection,
    task_id: &str,
    dir: &Path,
    source_root: &Path,
    now: i64,
) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            insert_recursive(conn, task_id, &path, source_root, now)?;
        } else if path.is_file() {
            let rel_path = path
                .strip_prefix(source_root)
                .map_err(|_| {
                    AppError::Internal("failed to compute relative path for state".to_string())
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = fs::metadata(&path)?;
            let size = metadata.len() as i64;
            let mtime = file_mtime_epoch(&path)?;
            conn.execute(
                "INSERT INTO task_file_state (task_id, rel_path, file_size, file_mtime, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![task_id, rel_path, size, mtime, now],
            )?;
        }
    }
    Ok(())
}

fn file_mtime_epoch(path: &Path) -> AppResult<i64> {
    let mtime = fs::metadata(path)?.modified().map_err(|e| {
        AppError::Internal(format!("failed to get mtime for {}: {e}", path.display()))
    })?;
    let secs = mtime
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Internal(format!("invalid mtime for {}: {e}", path.display())))?;
    Ok(secs.as_secs() as i64)
}

fn required_file_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path file name must be valid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::TempDir;

    #[test]
    fn records_directory_state_and_skips_unchanged_file() {
        let db = Database::open_in_memory().expect("open in-memory database");
        let conn = db.connection().expect("open connection");
        insert_task(&conn, "task-1");
        let temp = TempDir::new().expect("create temp dir");
        let source = temp.path().join("source");
        let nested = source.join("nested");
        fs::create_dir_all(&nested).expect("create source dir");
        let file = nested.join("a.md");
        fs::write(&file, "# A\n").expect("write file");

        FileState::record(&conn, "task-1", &source).expect("record state");
        let state = FileState::load(&conn, "task-1").expect("load state");

        assert!(state.contains_key("nested/a.md"));
        assert!(
            FileState::should_skip(&file, "nested/a.md", &state).expect("check skip"),
            "unchanged file should be skipped"
        );
    }

    #[test]
    fn record_single_file_uses_file_name_as_relative_path() {
        let db = Database::open_in_memory().expect("open in-memory database");
        let conn = db.connection().expect("open connection");
        insert_task(&conn, "task-1");
        let temp = TempDir::new().expect("create temp dir");
        let file = temp.path().join("session.md");
        fs::write(&file, "# Session\n").expect("write file");

        FileState::record(&conn, "task-1", &file).expect("record state");
        let state = FileState::load(&conn, "task-1").expect("load state");

        assert!(state.contains_key("session.md"));
    }

    fn insert_task(conn: &rusqlite::Connection, task_id: &str) {
        conn.execute(
            "INSERT INTO task_groups (id, name, created_at, updated_at)
             VALUES ('g1', 'Group', 0, 0)",
            [],
        )
        .expect("insert group");
        conn.execute(
            "INSERT INTO tasks (
                id, group_id, direction, action, source_type, source, target_type, target,
                schedule, last_status, created_at, updated_at
             )
             VALUES (?1, 'g1', 'Push', 'Copy', 'Local', '/tmp/source', 'Cloud', 'backup/',
                     'manual', 'never', 0, 0)",
            [task_id],
        )
        .expect("insert task");
    }
}
