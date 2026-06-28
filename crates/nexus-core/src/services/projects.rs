use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::agent_capability_surfaces,
    services::paths,
};

/// Default Session Directory leaf restored when the override is cleared.
const DEFAULT_SESSIONS_DIR: &str = "__sessions";

/// Custom skills dir a new Project inherits when no global default is configured.
/// Mirrors the `projects.custom_skills_dirs` column default so behavior is unchanged
/// until the user edits the Project Defaults.
const DEFAULT_CUSTOM_SKILLS_DIR: &str = "skills";

/// Settings keys for the global Project Defaults applied at new-project creation.
const DEFAULT_CUSTOM_SKILLS_DIRS_KEY: &str = "DEFAULT_CUSTOM_SKILLS_DIRS";
const DEFAULT_EXTRA_PROMPT_FILES_KEY: &str = "DEFAULT_EXTRA_PROMPT_FILES";
const DEFAULT_SESSIONS_DIR_KEY: &str = "DEFAULT_SESSIONS_DIR";

/// Columns shared by every Project read, in the order `project_from_row` expects.
const PROJECT_SELECT_COLUMNS: &str = r#"
    p.id,
    p.name,
    p.status,
    p.path,
    p.sessions_dir,
    NULL AS sessions_note,
    (SELECT COUNT(*) FROM skills WHERE project_id = p.id) AS skills,
    (SELECT COUNT(*) FROM session_index WHERE project_id = p.id) AS sessions,
    0 AS sync,
    p.key,
    p.custom_skills_dirs,
    (SELECT COUNT(*) FROM prompts WHERE project_id = p.id) AS prompts,
    p.extra_prompt_files
"#;

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
    pub prompts: i64,
    pub sessions: i64,
    pub sync: i64,
    pub key: String,
    /// Project custom skills directories (relative to Project root, or absolute).
    /// Scanned as Project custom sources alongside the fixed Agent project skills dirs.
    pub custom_skills_dirs: Vec<String>,
    /// Project extra prompt files (relative to Project root). Scanned alongside the
    /// primary AGENTS.md / CLAUDE.md; each filename must match an Agent prompt glob.
    pub extra_prompt_files: Vec<String>,
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

/// Global defaults a brand-new `Project` inherits at creation. They are a snapshot
/// applied once in `record_project`; later edits never retro-apply to existing
/// projects, which keep their own per-Project overrides. Validated by the same
/// rules as the per-Project setters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaults {
    pub custom_skills_dirs: Vec<String>,
    pub extra_prompt_files: Vec<String>,
    pub sessions_dir: String,
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
        let mut stmt = conn.prepare(&format!(
            r#"
            SELECT {PROJECT_SELECT_COLUMNS}
            FROM projects p
            ORDER BY p.sort_index IS NULL, p.sort_index, p.created_at, p.name
            "#,
        ))?;

        let rows = stmt.query_map([], project_from_row)?;
        let mut projects = rows.collect::<Result<Vec<_>, _>>()?;

        for project in &mut projects {
            if project.status == "active" && !Path::new(&project.path).exists() {
                project.status = "stale".to_string();
            }
        }

        Ok(projects)
    }

    pub fn reorder_projects(&self, project_ids: Vec<String>) -> AppResult<Vec<Project>> {
        let project_ids = normalize_project_order(project_ids)?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        let mut stmt = tx.prepare("SELECT id FROM projects")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let existing_ids = rows.collect::<Result<HashSet<_>, _>>()?;
        drop(stmt);

        if project_ids.len() != existing_ids.len() {
            return Err(AppError::Validation(
                "project order must include every project exactly once".to_string(),
            ));
        }

        for id in &project_ids {
            if !existing_ids.contains(id) {
                return Err(AppError::Validation(format!(
                    "project order contains unknown project id: {id}"
                )));
            }
        }

        let now = now_epoch_seconds()?;
        for (index, id) in project_ids.iter().enumerate() {
            tx.execute(
                "UPDATE projects SET sort_index = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, index as i64, now],
            )?;
        }

        tx.commit()?;
        drop(conn);
        self.list_projects()
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
                let defaults = self.read_project_defaults(&tx)?;
                let id = Uuid::new_v4().to_string();
                tx.execute(
                    r#"
                    INSERT INTO projects (
                        id, name, key, path, status,
                        sessions_dir, custom_skills_dirs, extra_prompt_files,
                        created_at, updated_at
                    )
                    VALUES (?1, ?2, ?2, ?3, 'active', ?4, ?5, ?6, ?7, ?7)
                    "#,
                    params![
                        id,
                        key,
                        path,
                        defaults.sessions_dir,
                        defaults.custom_skills_dirs.join("\n"),
                        defaults.extra_prompt_files.join("\n"),
                        now,
                    ],
                )?;
                id
            }
        };

        let project = tx.query_row(
            &format!(
                r#"
                SELECT {PROJECT_SELECT_COLUMNS}
                FROM projects p
                WHERE p.id = ?1
                "#,
            ),
            params![id],
            project_from_row,
        )?;

        tx.commit()?;
        Ok(project)
    }

    /// Replace the full set of Project custom skills directories. Entries are trimmed
    /// and de-duplicated by normalized identity; a dir resolving to a fixed Agent
    /// project skills dir is rejected so the same path never yields two source kinds.
    pub fn set_project_custom_skills_dirs(
        &self,
        project_id: String,
        dirs: Vec<String>,
    ) -> AppResult<Project> {
        let id = project_id.trim();
        if id.is_empty() {
            return Err(AppError::Validation("project id is required".to_string()));
        }

        let value = validate_custom_skills_dirs(dirs)?.join("\n");
        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        let changed = conn.execute(
            "UPDATE projects SET custom_skills_dirs = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, value, now],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("project was not found".to_string()));
        }

        conn.query_row(
            &format!(
                r#"
                SELECT {PROJECT_SELECT_COLUMNS}
                FROM projects p
                WHERE p.id = ?1
                "#,
            ),
            params![id],
            project_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("project was not found".to_string()))
    }

    /// Replace the full set of Project extra prompt files. Entries are trimmed and
    /// de-duplicated by normalized identity; a file whose name does not match an Agent
    /// prompt glob (`AGENTS*.md` / `CLAUDE*.md`) is rejected, as is one that collides
    /// with an auto-discovered primary project prompt file (`AGENTS.md` / `CLAUDE.md`).
    pub fn set_project_extra_prompt_files(
        &self,
        project_id: String,
        files: Vec<String>,
    ) -> AppResult<Project> {
        let id = project_id.trim();
        if id.is_empty() {
            return Err(AppError::Validation("project id is required".to_string()));
        }

        let value = validate_extra_prompt_files(files)?.join("\n");
        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        let changed = conn.execute(
            "UPDATE projects SET extra_prompt_files = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, value, now],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("project was not found".to_string()));
        }

        conn.query_row(
            &format!(
                r#"
                SELECT {PROJECT_SELECT_COLUMNS}
                FROM projects p
                WHERE p.id = ?1
                "#,
            ),
            params![id],
            project_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("project was not found".to_string()))
    }

    /// Override the Project Session Directory. An empty input restores the default
    /// `__sessions` template. Session Directory stays single-valued by design.
    pub fn set_project_sessions_dir(&self, project_id: String, dir: String) -> AppResult<Project> {
        let id = project_id.trim();
        if id.is_empty() {
            return Err(AppError::Validation("project id is required".to_string()));
        }

        let value = normalize_sessions_dir(&dir);

        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        let changed = conn.execute(
            "UPDATE projects SET sessions_dir = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, value, now],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("project was not found".to_string()));
        }

        conn.query_row(
            &format!(
                r#"
                SELECT {PROJECT_SELECT_COLUMNS}
                FROM projects p
                WHERE p.id = ?1
                "#,
            ),
            params![id],
            project_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("project was not found".to_string()))
    }

    /// Read the global Project Defaults applied to brand-new projects. Unset list
    /// settings fall back to their column defaults (`skills` for custom skills dirs,
    /// none for extra prompt files); an unset session dir falls back to `__sessions`.
    pub fn get_project_defaults(&self) -> AppResult<ProjectDefaults> {
        let conn = self.db.connection()?;
        Ok(read_project_defaults_from(&conn)?)
    }

    /// Replace the default custom skills dirs new projects inherit. Validated by the
    /// same rules as the per-Project setter (dedup, reject fixed Agent dirs).
    pub fn set_default_custom_skills_dirs(
        &self,
        dirs: Vec<String>,
    ) -> AppResult<ProjectDefaults> {
        let value = validate_custom_skills_dirs(dirs)?.join("\n");
        let conn = self.db.connection()?;
        write_setting(&conn, DEFAULT_CUSTOM_SKILLS_DIRS_KEY, &value)?;
        Ok(read_project_defaults_from(&conn)?)
    }

    /// Replace the default extra prompt files new projects inherit. Validated by the
    /// same rules as the per-Project setter (prompt glob, no primary-file collision).
    pub fn set_default_extra_prompt_files(
        &self,
        files: Vec<String>,
    ) -> AppResult<ProjectDefaults> {
        let value = validate_extra_prompt_files(files)?.join("\n");
        let conn = self.db.connection()?;
        write_setting(&conn, DEFAULT_EXTRA_PROMPT_FILES_KEY, &value)?;
        Ok(read_project_defaults_from(&conn)?)
    }

    /// Replace the default Session Directory new projects inherit. An empty input
    /// restores the `__sessions` default.
    pub fn set_default_sessions_dir(&self, dir: String) -> AppResult<ProjectDefaults> {
        let value = normalize_sessions_dir(&dir);
        let conn = self.db.connection()?;
        write_setting(&conn, DEFAULT_SESSIONS_DIR_KEY, &value)?;
        Ok(read_project_defaults_from(&conn)?)
    }

    fn read_project_defaults(&self, conn: &rusqlite::Connection) -> AppResult<ProjectDefaults> {
        read_project_defaults_from(conn).map_err(Into::into)
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

    pub fn delete_project(&self, id: String) -> AppResult<()> {
        if id.trim().is_empty() {
            return Err(AppError::Validation("project id is required".to_string()));
        }

        let conn = self.db.connection()?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id.trim()])?;

        Ok(())
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

fn normalize_project_order(project_ids: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(project_ids.len());
    let mut seen = HashSet::with_capacity(project_ids.len());

    for id in project_ids {
        let id = id.trim().to_string();
        if id.is_empty() {
            return Err(AppError::Validation(
                "project order contains an empty project id".to_string(),
            ));
        }
        if !seen.insert(id.clone()) {
            return Err(AppError::Validation(format!(
                "project order contains duplicate project id: {id}"
            )));
        }
        normalized.push(id);
    }

    Ok(normalized)
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
        custom_skills_dirs: parse_dir_list(&row.get::<_, String>(10)?),
        prompts: row.get(11)?,
        extra_prompt_files: parse_dir_list(&row.get::<_, String>(12)?),
    })
}

/// Split a newline-joined custom skills dir list into trimmed, non-empty entries.
pub(crate) fn parse_dir_list(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Normalize a custom skills dir for de-duplication and conflict checks: trim, use
/// `/` separators, drop a leading `./` and any trailing slash. Identity only — the
/// stored value keeps the user's original (relative or absolute) form.
pub(crate) fn normalize_custom_dir(dir: &str) -> String {
    let normalized = dir.trim().replace('\\', "/");
    let normalized = normalized.strip_prefix("./").unwrap_or(&normalized);
    normalized.trim_end_matches('/').to_string()
}

/// Trim, normalize, and de-duplicate a custom skills dir list, rejecting any dir
/// that resolves to a fixed Agent project skills dir. Returns the stored entries in
/// their original form. Shared by the per-Project setter and the Project Defaults.
fn validate_custom_skills_dirs(dirs: Vec<String>) -> AppResult<Vec<String>> {
    let agent_dirs: HashSet<String> = agent_capability_surfaces()
        .iter()
        .filter_map(|agent| {
            agent
                .skill
                .map(|skill| normalize_custom_dir(skill.project_dir))
        })
        .collect();

    let mut seen = HashSet::new();
    let mut stored = Vec::new();
    for dir in dirs {
        let trimmed = dir.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = normalize_custom_dir(trimmed);
        if normalized.is_empty() {
            continue;
        }
        if agent_dirs.contains(&normalized) {
            return Err(AppError::Validation(format!(
                "custom skills dir conflicts with a fixed agent skills dir: {normalized}"
            )));
        }
        if seen.insert(normalized) {
            stored.push(trimmed.to_string());
        }
    }

    Ok(stored)
}

/// Trim, normalize, and de-duplicate an extra prompt file list, rejecting files that
/// do not match a prompt glob (`AGENTS*.md` / `CLAUDE*.md`) or collide with an
/// auto-discovered primary prompt file. Shared by the per-Project setter and the
/// Project Defaults.
fn validate_extra_prompt_files(files: Vec<String>) -> AppResult<Vec<String>> {
    let primary_files: HashSet<String> = prompt_primary_files();

    let mut seen = HashSet::new();
    let mut stored = Vec::new();
    for file in files {
        let trimmed = file.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = normalize_custom_dir(trimmed);
        if normalized.is_empty() {
            continue;
        }
        if primary_files.contains(&normalized) {
            return Err(AppError::Validation(format!(
                "extra prompt file collides with an auto-discovered primary prompt file: {normalized}"
            )));
        }
        if !matches_prompt_glob(&normalized) {
            return Err(AppError::Validation(format!(
                "extra prompt file does not match a prompt glob (AGENTS*.md / CLAUDE*.md): {trimmed}"
            )));
        }
        if seen.insert(normalized) {
            stored.push(trimmed.to_string());
        }
    }

    Ok(stored)
}

/// Normalize a Session Directory override: an empty input restores the `__sessions`
/// default, otherwise the value is normalized like any other custom dir.
fn normalize_sessions_dir(dir: &str) -> String {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        DEFAULT_SESSIONS_DIR.to_string()
    } else {
        normalize_custom_dir(trimmed)
    }
}

/// Read the global Project Defaults from `settings`, falling back to the same column
/// defaults a project would otherwise get. Unset list keys distinguish from a stored
/// empty list: no row means "inherit the column default", an empty row means "none".
fn read_project_defaults_from(conn: &rusqlite::Connection) -> rusqlite::Result<ProjectDefaults> {
    let custom_skills_dirs = match read_setting(conn, DEFAULT_CUSTOM_SKILLS_DIRS_KEY)? {
        Some(value) => parse_dir_list(&value),
        None => vec![DEFAULT_CUSTOM_SKILLS_DIR.to_string()],
    };
    let extra_prompt_files = read_setting(conn, DEFAULT_EXTRA_PROMPT_FILES_KEY)?
        .map(|value| parse_dir_list(&value))
        .unwrap_or_default();
    let sessions_dir = read_setting(conn, DEFAULT_SESSIONS_DIR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SESSIONS_DIR.to_string());

    Ok(ProjectDefaults {
        custom_skills_dirs,
        extra_prompt_files,
        sessions_dir,
    })
}

fn read_setting(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn write_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// The auto-discovered primary project prompt filenames (`AGENTS.md` / `CLAUDE.md`).
/// Extra prompt files must not collide with these — they are already scanned.
fn prompt_primary_files() -> HashSet<String> {
    agent_capability_surfaces()
        .iter()
        .filter_map(|agent| agent.prompt.and_then(|prompt| prompt.project_file))
        .map(|file| file.to_string())
        .collect()
}

/// True when a normalized file path's basename matches an Agent prompt glob — that is,
/// it starts with a primary stem (`AGENTS` / `CLAUDE`) and ends with `.md`.
fn matches_prompt_glob(file: &str) -> bool {
    let base = file.rsplit('/').next().unwrap_or(file);
    if !base.ends_with(".md") {
        return false;
    }
    prompt_file_stems()
        .iter()
        .any(|stem| base.starts_with(stem))
}

/// Prompt-file stems derived from each Agent's primary project prompt file
/// (`AGENTS.md` → `AGENTS`, `CLAUDE.md` → `CLAUDE`).
fn prompt_file_stems() -> Vec<String> {
    agent_capability_surfaces()
        .iter()
        .filter_map(|agent| agent.prompt.and_then(|prompt| prompt.project_file))
        .filter_map(|file| file.strip_suffix(".md").map(ToOwned::to_owned))
        .collect()
}

fn git_base_folder_from_row(row: &Row<'_>) -> rusqlite::Result<GitBaseFolder> {
    Ok(GitBaseFolder {
        id: row.get(0)?,
        path: row.get(1)?,
        added_at: row.get(2)?,
    })
}
