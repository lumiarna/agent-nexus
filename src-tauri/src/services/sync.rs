use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::paths,
    services::symlink::{create_symlink_placement, remove_symlink, remove_symlink_if_present},
    services::webdav,
};

const PROJECT_SYMLINK_IGNORED_DIRS_SETTING: &str = "sync_project_symlink_ignored_dirs";
const DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &[&str] = &[".git", ".venv", "node_modules"];
const WEBDAV_URL_SETTING: &str = "webdav_url";
const WEBDAV_USER_SETTING: &str = "webdav_user";
const WEBDAV_PASS_SETTING: &str = "webdav_pass";
const WEBDAV_REMOTE_ROOT_SETTING: &str = "webdav_remote_root";
const DEFAULT_WEBDAV_REMOTE_ROOT: &str = "agent-nexus-sync";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSymlink {
    pub id: String,
    pub source_path: String,
    pub source_project_id: Option<String>,
    pub source_project_name: Option<String>,
    pub target_path: String,
    pub target_project_id: Option<String>,
    pub target_project_name: Option<String>,
    pub link_kind: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub direction: String,
    pub action: String,
    pub source_type: String,
    pub source: String,
    pub target_type: String,
    pub target: String,
    pub schedule: String,
    pub last_run: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
    pub id: String,
    pub name: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub action: String,
    pub source_type: String,
    pub source: String,
    pub target_type: String,
    pub target: String,
    pub schedule: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskGroupInput {
    pub name: String,
    pub tasks: Vec<CreateTaskInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebdavSettings {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub remote_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebdavSettingsInput {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub remote_root: String,
}

struct PreparedTask {
    action: String,
    source_type: String,
    source: String,
    target_type: String,
    target: String,
    schedule: String,
    direction: String,
}

#[derive(Clone)]
pub struct SyncService {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct ProjectRoot {
    id: String,
    name: String,
    path: PathBuf,
}

impl SyncService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn get_webdav_settings(&self) -> AppResult<WebdavSettings> {
        let conn = self.db.connection()?;
        Ok(WebdavSettings {
            url: read_setting(&conn, WEBDAV_URL_SETTING)?.unwrap_or_default(),
            user: read_setting(&conn, WEBDAV_USER_SETTING)?.unwrap_or_default(),
            pass: read_setting(&conn, WEBDAV_PASS_SETTING)?.unwrap_or_default(),
            remote_root: read_setting(&conn, WEBDAV_REMOTE_ROOT_SETTING)?
                .unwrap_or_else(|| DEFAULT_WEBDAV_REMOTE_ROOT.to_string()),
        })
    }

    pub fn save_webdav_settings(&self, input: WebdavSettingsInput) -> AppResult<WebdavSettings> {
        let settings = normalize_webdav_settings(input)?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        upsert_setting(&tx, WEBDAV_URL_SETTING, &settings.url)?;
        upsert_setting(&tx, WEBDAV_USER_SETTING, &settings.user)?;
        upsert_setting(&tx, WEBDAV_PASS_SETTING, &settings.pass)?;
        upsert_setting(&tx, WEBDAV_REMOTE_ROOT_SETTING, &settings.remote_root)?;
        tx.commit()?;

        Ok(settings)
    }

    pub async fn test_webdav_connection(&self, input: WebdavSettingsInput) -> AppResult<()> {
        let settings = normalize_webdav_settings(input)?;
        let auth = webdav::auth_from_credentials(&settings.user, &settings.pass);
        webdav::test_connection(&settings.url, &auth).await?;
        let segments = webdav::path_segments(&settings.remote_root)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        webdav::ensure_remote_directories(&settings.url, &segments, &auth).await
    }

    pub async fn run_task(&self, id: String) -> AppResult<Task> {
        let id = required_trimmed(&id, "task id")?.to_string();
        let task = self
            .find_task(&id)?
            .ok_or_else(|| AppError::Validation("task not found".to_string()))?;

        let result = self.run_task_operation(&task).await;
        match result {
            Ok(()) => self.record_task_run(&id, "ok")?,
            Err(error) => {
                let _ = self.record_task_run(&id, "failed");
                return Err(error);
            }
        }

        self.find_task(&id)?
            .ok_or_else(|| AppError::Internal("completed task was not found".to_string()))
    }

    pub fn list_project_symlinks(&self) -> AppResult<Vec<ProjectSymlink>> {
        let projects = self.list_existing_project_roots()?;
        let ignored_dirs = self.project_symlink_ignored_dirs()?;
        let mut links = Vec::new();
        let mut seen_targets = HashSet::new();

        for project in &projects {
            collect_project_symlinks(
                project,
                &projects,
                &ignored_dirs,
                &mut seen_targets,
                &mut links,
            )?;
        }

        links.sort_by(|left, right| {
            left.target_path
                .cmp(&right.target_path)
                .then_with(|| left.source_path.cmp(&right.source_path))
        });
        Ok(links)
    }

    pub fn list_task_groups(&self) -> AppResult<Vec<TaskGroup>> {
        let conn = self.db.connection()?;
        let mut group_stmt = conn.prepare(
            r#"
            SELECT id, name
            FROM task_groups
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let group_rows = group_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut groups = Vec::new();
        for group_row in group_rows {
            let (id, name) = group_row?;
            groups.push(TaskGroup {
                tasks: list_tasks_for_group(&conn, &id)?,
                id,
                name,
            });
        }

        Ok(groups)
    }

    pub fn create_task_group(&self, input: CreateTaskGroupInput) -> AppResult<TaskGroup> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AppError::Validation(
                "task group name is required".to_string(),
            ));
        }
        if input.tasks.is_empty() {
            return Err(AppError::Validation(
                "at least one task is required".to_string(),
            ));
        }

        let tasks = input
            .tasks
            .iter()
            .map(prepare_task)
            .collect::<AppResult<Vec<_>>>()?;
        let created_symlinks = create_symlink_placements(&tasks)?;

        let result = (|| -> AppResult<TaskGroup> {
            let now = now_epoch_seconds()?;
            let group_id = Uuid::new_v4().to_string();
            let mut conn = self.db.connection()?;
            let tx = conn.transaction()?;

            tx.execute(
                r#"
                INSERT INTO task_groups (id, name, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?3)
                "#,
                params![group_id, name, now],
            )?;

            for (index, task) in tasks.iter().enumerate() {
                tx.execute(
                    r#"
                    INSERT INTO tasks (
                        id, group_id, direction, action, source_type, source, target_type, target,
                        schedule, sort_index, last_status, created_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'never', ?11, ?11)
                    "#,
                    params![
                        Uuid::new_v4().to_string(),
                        group_id,
                        task.direction,
                        task.action,
                        task.source_type,
                        task.source,
                        task.target_type,
                        task.target,
                        task.schedule,
                        index as i64,
                        now,
                    ],
                )?;
            }

            tx.commit()?;
            drop(conn);

            self.list_task_groups()?
                .into_iter()
                .find(|group| group.id == group_id)
                .ok_or_else(|| AppError::Internal("created task group was not found".to_string()))
        })();

        if result.is_err() {
            remove_created_symlinks(&created_symlinks);
        }

        result
    }

    pub fn delete_task(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "task id")?;
        let conn = self.db.connection()?;
        let task = conn
            .query_row(
                r#"
                SELECT action, target_type, target
                FROM tasks
                WHERE id = ?1
                "#,
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let Some((action, target_type, target)) = task else {
            return Ok(());
        };

        if action == "Symlink" && target_type == "Local" {
            remove_symlink_if_present(Path::new(&target))?;
        }

        conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_project_symlink(&self, target_path: String) -> AppResult<()> {
        let target_path = required_trimmed(&target_path, "project symlink target path")?;
        remove_symlink(Path::new(target_path))
    }

    async fn run_task_operation(&self, task: &Task) -> AppResult<()> {
        if task.action != "Copy" {
            return Err(AppError::Validation(
                "only Copy tasks can be run manually".to_string(),
            ));
        }

        match (task.source_type.as_str(), task.target_type.as_str()) {
            ("Local", "Cloud") => {
                let settings = self.valid_webdav_settings()?;
                push_local_to_cloud(task, &settings).await
            }
            ("Cloud", "Local") => Err(AppError::Validation(
                "Cloud to Local copy is not implemented yet".to_string(),
            )),
            ("Local", "Local") => Err(AppError::Validation(
                "Local to Local copy is not implemented yet".to_string(),
            )),
            _ => Err(AppError::Validation(
                "Cloud to Cloud copy is not supported".to_string(),
            )),
        }
    }

    fn valid_webdav_settings(&self) -> AppResult<WebdavSettings> {
        let settings = self.get_webdav_settings()?;
        normalize_webdav_settings(WebdavSettingsInput {
            url: settings.url,
            user: settings.user,
            pass: settings.pass,
            remote_root: settings.remote_root,
        })
    }

    fn find_task(&self, id: &str) -> AppResult<Option<Task>> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT
                id,
                direction,
                action,
                source_type,
                source,
                target_type,
                target,
                schedule,
                COALESCE(strftime('%m-%d %H:%M', last_run_at, 'unixepoch'), '—') AS last_run,
                COALESCE(last_status, 'never') AS status
            FROM tasks
            WHERE id = ?1
            "#,
            [id],
            task_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    fn record_task_run(&self, id: &str, status: &str) -> AppResult<()> {
        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        conn.execute(
            r#"
            UPDATE tasks
            SET last_run_at = ?2, last_status = ?3, updated_at = ?2
            WHERE id = ?1
            "#,
            params![id, now, status],
        )?;
        Ok(())
    }

    fn list_existing_project_roots(&self) -> AppResult<Vec<ProjectRoot>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, path
            FROM projects
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;

        let rows = stmt.query_map([], project_root_from_row)?;
        let mut projects = Vec::new();

        for row in rows {
            let project = row?;
            if project.path.is_dir() {
                projects.push(project);
            }
        }

        Ok(projects)
    }

    fn project_symlink_ignored_dirs(&self) -> AppResult<HashSet<String>> {
        let conn = self.db.connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [PROJECT_SYMLINK_IGNORED_DIRS_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        Ok(match value {
            Some(value) => parse_ignored_dirs(&value),
            None => DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
                .iter()
                .map(|value| value.to_string())
                .collect(),
        })
    }
}

fn prepare_task(task: &CreateTaskInput) -> AppResult<PreparedTask> {
    let action = validate_one_of(&task.action, &["Symlink", "Copy"], "task action")?;
    let source_type = validate_one_of(&task.source_type, &["Local", "Cloud"], "source type")?;
    let target_type = validate_one_of(&task.target_type, &["Local", "Cloud"], "target type")?;
    if source_type == "Cloud" && target_type == "Cloud" {
        return Err(AppError::Validation(
            "Cloud to Cloud sync tasks are not supported".to_string(),
        ));
    }
    let source = required_trimmed(&task.source, "task source")?;
    let target = required_trimmed(&task.target, "task target")?;
    if action == "Symlink" && (source_type != "Local" || target_type != "Local") {
        return Err(AppError::Validation(
            "symlink tasks require local source and target".to_string(),
        ));
    }
    let schedule = task.schedule.trim();
    let schedule = if schedule.is_empty() {
        "manual"
    } else {
        schedule
    };
    if schedule != "manual" {
        return Err(AppError::Validation(
            "scheduled sync tasks are not implemented yet".to_string(),
        ));
    }
    let direction = derive_direction(source_type, target_type);

    Ok(PreparedTask {
        action: action.to_string(),
        source_type: source_type.to_string(),
        source: source.to_string(),
        target_type: target_type.to_string(),
        target: target.to_string(),
        schedule: schedule.to_string(),
        direction: direction.to_string(),
    })
}

fn normalize_webdav_url(raw: &str) -> AppResult<String> {
    let value = required_trimmed(raw, "WebDAV URL")?;
    let url = Url::parse(value)
        .map_err(|error| AppError::Validation(format!("invalid WebDAV URL: {error}")))?;
    match url.scheme() {
        "http" | "https" => Ok(value.to_string()),
        _ => Err(AppError::Validation(
            "WebDAV URL must use http or https".to_string(),
        )),
    }
}

fn normalize_webdav_remote_root(raw: &str) -> String {
    let value = raw.trim().trim_matches('/');
    if value.is_empty() {
        DEFAULT_WEBDAV_REMOTE_ROOT.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_webdav_settings(input: WebdavSettingsInput) -> AppResult<WebdavSettings> {
    Ok(WebdavSettings {
        url: normalize_webdav_url(&input.url)?,
        user: input.user.trim().to_string(),
        pass: input.pass,
        remote_root: normalize_webdav_remote_root(&input.remote_root),
    })
}

async fn push_local_to_cloud(task: &Task, settings: &WebdavSettings) -> AppResult<()> {
    let source = Path::new(&task.source);
    let auth = webdav::auth_from_credentials(&settings.user, &settings.pass);

    if source.is_file() {
        push_local_file_to_cloud(source, &task.target, settings, &auth).await
    } else if source.is_dir() {
        push_local_directory_to_cloud(source, &task.target, settings, &auth).await
    } else {
        Err(AppError::Validation(format!(
            "local source does not exist: {}",
            task.source
        )))
    }
}

async fn push_local_file_to_cloud(
    source: &Path,
    target: &str,
    settings: &WebdavSettings,
    auth: &webdav::WebdavAuth,
) -> AppResult<()> {
    let mut file_segments = remote_segments(settings, target)?;
    if target.trim().ends_with('/') {
        file_segments.push(required_file_name(source)?);
    }
    if file_segments.len() < 2 {
        return Err(AppError::Validation(
            "cloud file target must include a file path".to_string(),
        ));
    }

    let parent_segments = file_segments[..file_segments.len() - 1].to_vec();
    webdav::ensure_remote_directories(&settings.url, &parent_segments, auth).await?;
    let url = webdav::build_remote_url(&settings.url, &file_segments)?;
    webdav::put_bytes(&url, auth, fs::read(source)?, "application/octet-stream").await
}

async fn push_local_directory_to_cloud(
    source: &Path,
    target: &str,
    settings: &WebdavSettings,
    auth: &webdav::WebdavAuth,
) -> AppResult<()> {
    let target_segments = remote_segments(settings, target)?;
    let mut directories = Vec::new();
    let mut files = Vec::new();
    collect_local_directory_push(source, target_segments, &mut directories, &mut files)?;

    for directory in directories {
        webdav::ensure_remote_directories(&settings.url, &directory, auth).await?;
    }

    for (path, file_segments) in files {
        let url = webdav::build_remote_url(&settings.url, &file_segments)?;
        webdav::put_bytes(&url, auth, fs::read(path)?, "application/octet-stream").await?;
    }

    Ok(())
}

fn collect_local_directory_push(
    source: &Path,
    target_segments: Vec<String>,
    directories: &mut Vec<Vec<String>>,
    files: &mut Vec<(PathBuf, Vec<String>)>,
) -> AppResult<()> {
    directories.push(target_segments.clone());

    let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let mut child_segments = target_segments.clone();
        child_segments.push(required_file_name(&path)?);
        let metadata = fs::metadata(&path)?;
        if metadata.is_dir() {
            collect_local_directory_push(&path, child_segments, directories, files)?;
        } else if metadata.is_file() {
            files.push((path, child_segments));
        }
    }

    Ok(())
}

fn remote_segments(settings: &WebdavSettings, cloud_path: &str) -> AppResult<Vec<String>> {
    let mut segments = webdav::path_segments(&settings.remote_root)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    segments.extend(webdav::path_segments(cloud_path).map(ToOwned::to_owned));
    if segments.is_empty() {
        Err(AppError::Validation(
            "cloud target path is required".to_string(),
        ))
    } else {
        Ok(segments)
    }
}

fn required_file_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path file name must be valid UTF-8".to_string()))
}

fn read_setting(conn: &rusqlite::Connection, key: &str) -> AppResult<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .map_err(Into::into)
}

fn upsert_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        r#"
        INSERT INTO settings (key, value)
        VALUES (?1, ?2)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        "#,
        params![key, value],
    )?;
    Ok(())
}

fn create_symlink_placements(tasks: &[PreparedTask]) -> AppResult<Vec<PathBuf>> {
    let mut created = Vec::new();

    for task in tasks {
        if task.action != "Symlink" {
            continue;
        }

        let source = Path::new(&task.source);
        let target = Path::new(&task.target);
        create_symlink_placement(source, target).inspect_err(|_| {
            remove_created_symlinks(&created);
        })?;
        created.push(target.to_path_buf());
    }

    Ok(created)
}

fn remove_created_symlinks(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = remove_symlink_if_present(path);
    }
}

fn list_tasks_for_group(conn: &rusqlite::Connection, group_id: &str) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            direction,
            action,
            source_type,
            source,
            target_type,
            target,
            schedule,
            COALESCE(strftime('%m-%d %H:%M', last_run_at, 'unixepoch'), '—') AS last_run,
            COALESCE(last_status, 'never') AS status
        FROM tasks
        WHERE group_id = ?1
        ORDER BY sort_index IS NULL, sort_index, created_at
        "#,
    )?;

    let rows = stmt.query_map([group_id], task_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        direction: row.get(1)?,
        action: row.get(2)?,
        source_type: row.get(3)?,
        source: row.get(4)?,
        target_type: row.get(5)?,
        target: row.get(6)?,
        schedule: row.get(7)?,
        last_run: row.get(8)?,
        status: row.get(9)?,
    })
}

fn derive_direction(source_type: &str, target_type: &str) -> &'static str {
    if source_type == "Local" && target_type == "Local" {
        "Distribution"
    } else if source_type == "Local" && target_type == "Cloud" {
        "Push"
    } else {
        "Pull"
    }
}

fn required_trimmed<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::Validation(format!("{label} is required")))
    } else {
        Ok(value)
    }
}

fn validate_one_of<'a>(value: &'a str, allowed: &[&str], label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(AppError::Validation(format!("invalid {label}")))
    }
}

fn now_epoch_seconds() -> AppResult<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::Internal(error.to_string()))?
        .as_secs() as i64)
}

fn collect_project_symlinks(
    project: &ProjectRoot,
    projects: &[ProjectRoot],
    ignored_dirs: &HashSet<String>,
    seen_targets: &mut HashSet<String>,
    links: &mut Vec<ProjectSymlink>,
) -> AppResult<()> {
    collect_symlinks_in_dir(
        &project.path,
        project,
        projects,
        ignored_dirs,
        seen_targets,
        links,
    )
}

fn collect_symlinks_in_dir(
    dir: &Path,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
    ignored_dirs: &HashSet<String>,
    seen_targets: &mut HashSet<String>,
    links: &mut Vec<ProjectSymlink>,
) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if ignored_dirs.contains(file_name.as_ref()) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            let target_path = path_to_string(&path)?;
            if seen_targets.insert(target_path.clone()) {
                links.push(project_symlink_from_path(
                    &path,
                    target_project,
                    projects,
                    target_path,
                )?);
            }
            continue;
        }

        if metadata.is_dir() {
            collect_symlinks_in_dir(
                &path,
                target_project,
                projects,
                ignored_dirs,
                seen_targets,
                links,
            )?;
        }
    }

    Ok(())
}

fn parse_ignored_dirs(value: &str) -> HashSet<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn project_symlink_from_path(
    path: &Path,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
    target_path: String,
) -> AppResult<ProjectSymlink> {
    let raw_source = fs::read_link(path)?;
    let resolved_source = if raw_source.is_absolute() {
        raw_source
    } else {
        path.parent()
            .map(|parent| parent.join(&raw_source))
            .unwrap_or(raw_source)
    };
    let source_exists = resolved_source.exists();
    let canonical_source = if source_exists {
        Some(resolved_source.canonicalize()?)
    } else {
        None
    };
    let source_for_display = match &canonical_source {
        Some(path) => paths::path_buf_for_comparison(path.clone(), "project symlink source path")?,
        None => resolved_source.clone(),
    };
    let source_project = projects
        .iter()
        .find(|project| source_for_display.starts_with(&project.path));
    let source_for_metadata = canonical_source.as_deref().unwrap_or(&resolved_source);

    Ok(ProjectSymlink {
        id: target_path.clone(),
        source_path: path_to_string(&source_for_display)?,
        source_project_id: source_project.map(|project| project.id.clone()),
        source_project_name: source_project.map(|project| project.name.clone()),
        target_path,
        target_project_id: Some(target_project.id.clone()),
        target_project_name: Some(target_project.name.clone()),
        link_kind: link_kind(source_for_metadata),
        status: if source_exists { "ok" } else { "missing" }.to_string(),
    })
}

fn link_kind(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => "directory".to_string(),
        Ok(metadata) if metadata.is_file() => "file".to_string(),
        Ok(_) => "other".to_string(),
        Err(_) => "missing".to_string(),
    }
}

fn path_to_string(path: &Path) -> AppResult<String> {
    paths::path_to_string(path, "path")
}

fn project_root_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRoot> {
    let path: String = row.get(2)?;
    Ok(ProjectRoot {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(path),
    })
}
