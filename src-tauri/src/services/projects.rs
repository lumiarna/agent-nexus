use std::{
    collections::HashSet,
    fs,
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
    services::paths,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBaseFolder {
    pub id: String,
    pub path: String,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredRepo {
    pub path: String,
    pub key: String,
    pub state: String,
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

    pub fn list_git_base_folders(&self) -> AppResult<Vec<GitBaseFolder>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id,
                path,
                COALESCE(strftime('%Y-%m-%d', added_at, 'unixepoch'), '') AS added_at
            FROM git_base_folders
            ORDER BY added_at, path
            "#,
        )?;

        let rows = stmt.query_map([], git_base_folder_from_row)?;
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

    pub fn record_git_base_folder(&self, path: String) -> AppResult<GitBaseFolder> {
        let canonical_path = validate_directory_path(&path, "git base folder path")?;
        let path = path_to_string(&canonical_path)?;
        let now = now_epoch_seconds()?;

        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        let existing_id = tx
            .query_row(
                "SELECT id FROM git_base_folders WHERE path = ?1",
                params![path],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let id = match existing_id {
            Some(id) => {
                tx.execute(
                    "UPDATE git_base_folders SET added_at = ?2 WHERE id = ?1",
                    params![id, now],
                )?;
                id
            }
            None => {
                let id = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO git_base_folders (id, path, added_at) VALUES (?1, ?2, ?3)",
                    params![id, path, now],
                )?;
                id
            }
        };

        let folder = tx.query_row(
            r#"
            SELECT
                id,
                path,
                COALESCE(strftime('%Y-%m-%d', added_at, 'unixepoch'), '') AS added_at
            FROM git_base_folders
            WHERE id = ?1
            "#,
            params![id],
            git_base_folder_from_row,
        )?;

        tx.commit()?;
        Ok(folder)
    }

    pub fn remove_git_base_folder(&self, id: String) -> AppResult<()> {
        if id.trim().is_empty() {
            return Err(AppError::Validation(
                "git base folder id is required".to_string(),
            ));
        }

        let conn = self.db.connection()?;
        conn.execute(
            "DELETE FROM git_base_folders WHERE id = ?1",
            params![id.trim()],
        )?;

        Ok(())
    }

    pub fn scan_git_base_folder(&self, path: String) -> AppResult<Vec<DiscoveredRepo>> {
        let base = validate_directory_path(&path, "git base folder path")?;
        self.mark_recorded_repositories(discover_git_repositories(&base)?)
    }

    pub fn scan_git_base_folders(&self) -> AppResult<Vec<DiscoveredRepo>> {
        let folders = self.list_git_base_folders()?;
        let mut repositories = Vec::new();

        for folder in folders {
            repositories.extend(discover_git_repositories(Path::new(&folder.path))?);
        }

        self.mark_recorded_repositories(repositories)
    }

    fn mark_recorded_repositories(
        &self,
        repositories: Vec<DiscoveredRepo>,
    ) -> AppResult<Vec<DiscoveredRepo>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare("SELECT key FROM projects")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let recorded_keys = rows.collect::<Result<HashSet<_>, _>>()?;

        Ok(repositories
            .into_iter()
            .map(|repo| DiscoveredRepo {
                state: if recorded_keys.contains(&repo.key) {
                    "recorded".to_string()
                } else {
                    "new".to_string()
                },
                ..repo
            })
            .collect())
    }
}

fn validate_git_project_root(path: &str) -> AppResult<PathBuf> {
    let raw_path = validate_directory_path(path, "project path")?;

    if !raw_path.join(".git").exists() {
        return Err(AppError::Validation(format!(
            "project path is not a Git repository root: {}",
            raw_path.display()
        )));
    }

    Ok(raw_path.canonicalize()?)
}

fn validate_directory_path(path: &str, label: &str) -> AppResult<PathBuf> {
    if path.trim().is_empty() {
        return Err(AppError::Validation(format!("{label} is required")));
    }

    let raw_path = Path::new(path.trim());
    if !raw_path.exists() {
        return Err(AppError::Validation(format!(
            "{label} does not exist: {}",
            raw_path.display()
        )));
    }

    if !raw_path.is_dir() {
        return Err(AppError::Validation(format!(
            "{label} is not a directory: {}",
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
    paths::path_to_string(path, "project path")
}

fn discover_git_repositories(base: &Path) -> AppResult<Vec<DiscoveredRepo>> {
    let mut repositories = Vec::new();

    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() || !path.join(".git").exists() {
            continue;
        }

        let canonical_path = path.canonicalize()?;
        repositories.push(DiscoveredRepo {
            key: project_key(&canonical_path)?,
            path: path_to_string(&canonical_path)?,
            state: "new".to_string(),
        });
    }

    repositories.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(repositories)
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

fn git_base_folder_from_row(row: &Row<'_>) -> rusqlite::Result<GitBaseFolder> {
    Ok(GitBaseFolder {
        id: row.get(0)?,
        path: row.get(1)?,
        added_at: row.get(2)?,
    })
}
