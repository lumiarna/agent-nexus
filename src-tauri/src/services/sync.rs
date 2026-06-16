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
};

const PROJECT_SYMLINK_IGNORED_DIRS_SETTING: &str = "sync_project_symlink_ignored_dirs";
const DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &[&str] = &[".git", ".venv", "node_modules"];

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
    if source_type == "Cloud" || target_type == "Cloud" {
        return Err(AppError::Validation(
            "cloud sync tasks are not implemented yet".to_string(),
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

fn create_symlink_placement(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::Validation(format!(
            "symlink source does not exist: {}",
            source.display()
        )));
    }
    if fs::symlink_metadata(target).is_ok() {
        return Err(AppError::Validation(format!(
            "symlink target already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    create_symlink(source, target)
}

#[cfg(unix)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(source: &Path, target: &Path) -> AppResult<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)?;
    } else {
        std::os::windows::fs::symlink_file(source, target)?;
    }
    Ok(())
}

fn remove_created_symlinks(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = remove_symlink_if_present(path);
    }
}

fn remove_symlink(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_symlink() {
        return Err(AppError::Validation(
            "project symlink target path must be a symlink".to_string(),
        ));
    }

    fs::remove_file(path)?;
    Ok(())
}

fn remove_symlink_if_present(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => remove_symlink(path),
        Ok(_) | Err(_) => Ok(()),
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
    let comparable_source = if source_exists {
        resolved_source.canonicalize()?
    } else {
        resolved_source.clone()
    };
    let source_project = projects
        .iter()
        .find(|project| comparable_source.starts_with(&project.path));

    Ok(ProjectSymlink {
        id: target_path.clone(),
        source_path: path_to_string(&comparable_source)?,
        source_project_id: source_project.map(|project| project.id.clone()),
        source_project_name: source_project.map(|project| project.name.clone()),
        target_path,
        target_project_id: Some(target_project.id.clone()),
        target_project_name: Some(target_project.name.clone()),
        link_kind: link_kind(&comparable_source),
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
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path must be valid UTF-8".to_string()))
}

fn project_root_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRoot> {
    let path: String = row.get(2)?;
    Ok(ProjectRoot {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(path),
    })
}
