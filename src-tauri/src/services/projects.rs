use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub status: String,
    pub path: String,
    pub sessions_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions_note: Option<String>,
    pub skills: i64,
    pub sessions: i64,
    pub sync: i64,
    pub key: String,
}

#[derive(Clone)]
pub struct ProjectService {
    db: Arc<Database>,
}

impl ProjectService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_projects(&self) -> AppResult<Vec<Project>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                p.id,
                p.name,
                p.status,
                p.path,
                p.sessions_dir,
                NULL AS sessions_note,
                (SELECT COUNT(*) FROM skills WHERE project_id = p.id) AS skills,
                (SELECT COUNT(*) FROM session_index WHERE project_id = p.id) AS sessions,
                0 AS sync,
                p.key
            FROM projects p
            ORDER BY p.sort_index IS NULL, p.sort_index, p.created_at, p.name
            "#,
        )?;

        let rows = stmt.query_map([], project_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn record_project(&self, path: String) -> AppResult<Project> {
        let canonical_path = validate_git_project_root(&path)?;
        let key = project_key(&canonical_path)?;
        let path = path_to_string(&canonical_path)?;
        let now = now_epoch_seconds()?;

        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        let existing_id = tx
            .query_row(
                "SELECT id FROM projects WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let id = match existing_id {
            Some(id) => {
                tx.execute(
                    r#"
                    UPDATE projects
                    SET name = ?2,
                        path = ?3,
                        status = 'active',
                        updated_at = ?4
                    WHERE id = ?1
                    "#,
                    params![id, key, path, now],
                )?;
                id
            }
            None => {
                let id = Uuid::new_v4().to_string();
                tx.execute(
                    r#"
                    INSERT INTO projects (
                        id, name, key, path, status, sessions_dir, created_at, updated_at
                    )
                    VALUES (?1, ?2, ?2, ?3, 'active', '__sessions', ?4, ?4)
                    "#,
                    params![id, key, path, now],
                )?;
                id
            }
        };

        let project = tx.query_row(
            r#"
            SELECT
                p.id,
                p.name,
                p.status,
                p.path,
                p.sessions_dir,
                NULL AS sessions_note,
                (SELECT COUNT(*) FROM skills WHERE project_id = p.id) AS skills,
                (SELECT COUNT(*) FROM session_index WHERE project_id = p.id) AS sessions,
                0 AS sync,
                p.key
            FROM projects p
            WHERE p.id = ?1
            "#,
            params![id],
            project_from_row,
        )?;

        tx.commit()?;
        Ok(project)
    }
}

fn validate_git_project_root(path: &str) -> AppResult<PathBuf> {
    if path.trim().is_empty() {
        return Err(AppError::Validation("project path is required".to_string()));
    }

    let raw_path = Path::new(path.trim());
    if !raw_path.exists() {
        return Err(AppError::Validation(format!(
            "project path does not exist: {}",
            raw_path.display()
        )));
    }

    if !raw_path.is_dir() {
        return Err(AppError::Validation(format!(
            "project path is not a directory: {}",
            raw_path.display()
        )));
    }

    if !raw_path.join(".git").exists() {
        return Err(AppError::Validation(format!(
            "project path is not a Git repository root: {}",
            raw_path.display()
        )));
    }

    Ok(raw_path.canonicalize()?)
}

fn project_key(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("project path has no valid folder name".to_string()))
}

fn path_to_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("project path must be valid UTF-8".to_string()))
}

fn now_epoch_seconds() -> AppResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(duration.as_secs() as i64)
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        path: row.get(3)?,
        sessions_dir: row.get(4)?,
        sessions_note: row.get(5)?,
        skills: row.get(6)?,
        sessions: row.get(7)?,
        sync: row.get(8)?,
        key: row.get(9)?,
    })
}
