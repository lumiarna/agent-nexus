use std::{
    collections::HashSet,
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use rusqlite::{params, OptionalExtension, Row};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{
        cron::{cron_schedule_matches, normalize_task_schedule},
        outbound_request_log::OutboundRequestLogger,
        paths::{collapse_home, resolve_local_path},
        placement::{
            create_task_link_placement, remove_created_task_link_placements,
            task_link_placement_for_task, task_link_placements_for_group, task_link_state,
        },
        util::{now_epoch_seconds, required_trimmed},
        webdav,
    },
};

use super::{
    file_state::{FileState, FileStateMap},
    normalize_webdav_settings, read_webdav_settings,
    session_backup_reconciler::SessionBackupReconciler,
    CreateTaskGroupInput, CreateTaskInput, SessionBackup, Task, TaskGroup, WebdavSettings,
    WebdavSettingsInput, SESSION_BACKUP_GROUP_ID,
};

#[derive(Clone)]
pub(super) struct TaskLifecycle {
    db: Arc<Database>,
    request_logger: OutboundRequestLogger,
    transfer: Arc<dyn Transfer>,
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

enum TaskRunStatus {
    Ok,
    Skipped,
}

type TransferFuture<'a> = Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;

trait Transfer: Send + Sync {
    fn push_local_to_cloud<'a>(
        &'a self,
        task: &'a Task,
        settings: &'a WebdavSettings,
        file_states: &'a FileStateMap,
    ) -> TransferFuture<'a>;

    fn pull_cloud_to_local<'a>(
        &'a self,
        task: &'a Task,
        settings: &'a WebdavSettings,
        file_states: &'a FileStateMap,
    ) -> TransferFuture<'a>;
}

struct WebdavTransfer {
    request_logger: OutboundRequestLogger,
}

impl Transfer for WebdavTransfer {
    fn push_local_to_cloud<'a>(
        &'a self,
        task: &'a Task,
        settings: &'a WebdavSettings,
        file_states: &'a FileStateMap,
    ) -> TransferFuture<'a> {
        Box::pin(push_local_to_cloud(
            task,
            settings,
            file_states,
            &self.request_logger,
        ))
    }

    fn pull_cloud_to_local<'a>(
        &'a self,
        task: &'a Task,
        settings: &'a WebdavSettings,
        file_states: &'a FileStateMap,
    ) -> TransferFuture<'a> {
        Box::pin(pull_cloud_to_local(
            task,
            settings,
            file_states,
            &self.request_logger,
        ))
    }
}

impl TaskLifecycle {
    pub(super) fn new(db: Arc<Database>, request_logger: OutboundRequestLogger) -> Self {
        Self {
            db,
            request_logger: request_logger.clone(),
            transfer: Arc::new(WebdavTransfer { request_logger }),
        }
    }

    pub(super) fn request_logger(&self) -> &OutboundRequestLogger {
        &self.request_logger
    }

    pub(super) fn list_task_groups(&self) -> AppResult<Vec<TaskGroup>> {
        let conn = self.db.connection()?;
        let mut group_stmt = conn.prepare(
            r#"
            SELECT id, name
            FROM task_groups
            WHERE system_kind IS NULL
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

    pub(super) fn list_session_backups(&self) -> AppResult<Vec<SessionBackup>> {
        let reconciler = SessionBackupReconciler::new(self.db.as_ref());
        reconciler.reconcile()?;

        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                p.key,
                t.id,
                t.direction,
                t.action,
                t.source_type,
                t.source,
                t.target_type,
                t.target,
                t.schedule,
                t.last_run_at,
                COALESCE(t.last_status, 'never')
            FROM tasks t
            JOIN projects p ON p.id = t.project_id
            WHERE t.group_id = ?1
            ORDER BY p.sort_index IS NULL, p.sort_index, p.created_at, p.name
            "#,
        )?;
        let rows = stmt.query_map([SESSION_BACKUP_GROUP_ID], |row| {
            Ok(SessionBackup {
                project_key: row.get(0)?,
                task: Task {
                    id: row.get(1)?,
                    direction: row.get(2)?,
                    action: row.get(3)?,
                    source_type: row.get(4)?,
                    // Display-only: collapse `$HOME` to `~`. Execution reads tasks via
                    // `task_from_row` (canonical) and re-resolves, so this never feeds fs ops.
                    source: collapse_home(&row.get::<_, String>(5)?),
                    target_type: row.get(6)?,
                    target: collapse_home(&row.get::<_, String>(7)?),
                    schedule: row.get(8)?,
                    last_run_at: row.get(9)?,
                    status: row.get(10)?,
                    link_state: "present".to_string(),
                },
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(super) fn create_task_group(&self, input: CreateTaskGroupInput) -> AppResult<TaskGroup> {
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
        let created_symlinks = create_link_placements(&tasks)?;

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

    pub(super) fn delete_task(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "task id")?;
        let conn = self.db.connection()?;
        let Some(placement) = task_link_placement_for_task(&conn, id)? else {
            return Ok(());
        };

        placement.remove_if_present()?;

        conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    pub(super) fn delete_task_group(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "task group id")?;
        let conn = self.db.connection()?;
        let placements = task_link_placements_for_group(&conn, id)?;

        for placement in &placements {
            placement.remove_if_present()?;
        }

        conn.execute("DELETE FROM task_groups WHERE id = ?1", [id])?;
        Ok(())
    }

    pub(super) fn add_task(&self, group_id: String, task: CreateTaskInput) -> AppResult<TaskGroup> {
        let group_id = required_trimmed(&group_id, "task group id")?.to_string();
        let prepared = prepare_task(&task)?;

        let next_sort_index = {
            let conn = self.db.connection()?;
            let exists = conn
                .query_row(
                    "SELECT 1 FROM task_groups WHERE id = ?1",
                    [&group_id],
                    |_| Ok(true),
                )
                .optional()?
                .is_some();
            if !exists {
                return Err(AppError::Validation("task group not found".to_string()));
            }
            let next: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_index), -1) + 1 FROM tasks WHERE group_id = ?1",
                    [&group_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            next
        };

        let created_link = create_single_link_placement(&prepared)?;
        let result = (|| -> AppResult<TaskGroup> {
            let now = now_epoch_seconds()?;
            let mut conn = self.db.connection()?;
            let tx = conn.transaction()?;
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
                    prepared.direction,
                    prepared.action,
                    prepared.source_type,
                    prepared.source,
                    prepared.target_type,
                    prepared.target,
                    prepared.schedule,
                    next_sort_index,
                    now,
                ],
            )?;
            tx.commit()?;
            drop(conn);

            self.list_task_groups()?
                .into_iter()
                .find(|group| group.id == group_id)
                .ok_or_else(|| AppError::Internal("updated task group was not found".to_string()))
        })();

        if result.is_err() {
            if let Some(target) = created_link {
                remove_created_task_link_placements(&[target]);
            }
        }

        result
    }

    pub(super) fn update_task_schedule(&self, id: String, schedule: String) -> AppResult<Task> {
        let id = required_trimmed(&id, "task id")?.to_string();
        let task = self
            .find_task(&id)?
            .ok_or_else(|| AppError::Validation("task not found".to_string()))?;
        let schedule = normalize_task_schedule(&schedule, &task.action)?;
        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        conn.execute(
            r#"
            UPDATE tasks
            SET schedule = ?2, updated_at = ?3
            WHERE id = ?1
            "#,
            params![id, schedule, now],
        )?;
        drop(conn);

        self.find_task(&id)?
            .ok_or_else(|| AppError::Internal("updated task was not found".to_string()))
    }

    /// Bulk-apply one schedule to every Copy task in the group. This is the "group schedule":
    /// re-applying it overrides any per-task schedules (last write wins). Non-Copy tasks own no
    /// schedule and are left untouched.
    pub(super) fn update_group_schedule(
        &self,
        group_id: String,
        schedule: String,
    ) -> AppResult<()> {
        let group_id = required_trimmed(&group_id, "task group id")?.to_string();
        let schedule = normalize_task_schedule(&schedule, "Copy")?;
        let now = now_epoch_seconds()?;
        let conn = self.db.connection()?;
        let group_exists = conn
            .query_row(
                "SELECT 1 FROM task_groups WHERE id = ?1",
                [&group_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !group_exists {
            return Err(AppError::Validation("task group not found".to_string()));
        }
        conn.execute(
            r#"
            UPDATE tasks
            SET schedule = ?2, updated_at = ?3
            WHERE group_id = ?1 AND action = 'Copy'
            "#,
            params![group_id, schedule, now],
        )?;
        Ok(())
    }

    /// Persist a new top-to-bottom order for the user's task groups by writing each group's
    /// position into `sort_index`. The order must list every non-system group exactly once.
    pub(super) fn reorder_task_groups(&self, group_ids: Vec<String>) -> AppResult<Vec<TaskGroup>> {
        let group_ids = normalize_order(group_ids, "task group")?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        let mut stmt = tx.prepare("SELECT id FROM task_groups WHERE system_kind IS NULL")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let existing_ids = rows.collect::<Result<HashSet<_>, _>>()?;
        drop(stmt);

        if group_ids.len() != existing_ids.len() {
            return Err(AppError::Validation(
                "task group order must include every group exactly once".to_string(),
            ));
        }
        for id in &group_ids {
            if !existing_ids.contains(id) {
                return Err(AppError::Validation(format!(
                    "task group order contains unknown group id: {id}"
                )));
            }
        }

        let now = now_epoch_seconds()?;
        for (index, id) in group_ids.iter().enumerate() {
            tx.execute(
                "UPDATE task_groups SET sort_index = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, index as i64, now],
            )?;
        }
        tx.commit()?;
        drop(conn);
        self.list_task_groups()
    }

    /// Persist a new order for the tasks within one group. The order must list every task in
    /// the group exactly once.
    pub(super) fn reorder_tasks(
        &self,
        group_id: String,
        task_ids: Vec<String>,
    ) -> AppResult<TaskGroup> {
        let group_id = required_trimmed(&group_id, "task group id")?.to_string();
        let task_ids = normalize_order(task_ids, "task")?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        let group_exists = tx
            .query_row(
                "SELECT 1 FROM task_groups WHERE id = ?1",
                [&group_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !group_exists {
            return Err(AppError::Validation("task group not found".to_string()));
        }

        let mut stmt = tx.prepare("SELECT id FROM tasks WHERE group_id = ?1")?;
        let rows = stmt.query_map([&group_id], |row| row.get::<_, String>(0))?;
        let existing_ids = rows.collect::<Result<HashSet<_>, _>>()?;
        drop(stmt);

        if task_ids.len() != existing_ids.len() {
            return Err(AppError::Validation(
                "task order must include every task in the group exactly once".to_string(),
            ));
        }
        for id in &task_ids {
            if !existing_ids.contains(id) {
                return Err(AppError::Validation(format!(
                    "task order contains unknown task id: {id}"
                )));
            }
        }

        let now = now_epoch_seconds()?;
        for (index, id) in task_ids.iter().enumerate() {
            tx.execute(
                "UPDATE tasks SET sort_index = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, index as i64, now],
            )?;
        }
        tx.commit()?;
        drop(conn);

        self.list_task_groups()?
            .into_iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| AppError::Internal("reordered task group was not found".to_string()))
    }

    pub(super) async fn run_task(&self, id: String) -> AppResult<Task> {
        let id = required_trimmed(&id, "task id")?.to_string();
        let task = self
            .find_task(&id)?
            .ok_or_else(|| AppError::Validation("task not found".to_string()))?;

        let result = self.run_task_operation(&task).await;
        match result {
            Ok(TaskRunStatus::Ok) => self.record_task_run(&id, "ok")?,
            Ok(TaskRunStatus::Skipped) => self.record_task_run(&id, "skipped")?,
            Err(error) => {
                let _ = self.record_task_run(&id, "failed");
                return Err(error);
            }
        }

        self.find_task(&id)?
            .ok_or_else(|| AppError::Internal("completed task was not found".to_string()))
    }

    pub(super) async fn run_due_scheduled_tasks(
        &self,
        now_epoch_seconds: i64,
    ) -> AppResult<Vec<Task>> {
        let reconciler = SessionBackupReconciler::new(self.db.as_ref());
        reconciler.reconcile()?;

        let minute_start = now_epoch_seconds - now_epoch_seconds.rem_euclid(60);
        let scheduled_tasks = {
            let conn = self.db.connection()?;
            list_scheduled_copy_tasks(&conn)?
        };
        let mut ran = Vec::new();

        for task in scheduled_tasks {
            if task
                .last_run_at
                .is_some_and(|last_run_at| last_run_at >= minute_start)
            {
                continue;
            }
            if !cron_schedule_matches(&task.schedule, minute_start)? {
                continue;
            }

            let status = match self.run_task_operation(&task).await {
                Ok(TaskRunStatus::Ok) => "ok",
                Ok(TaskRunStatus::Skipped) => "skipped",
                Err(_) => "failed",
            };
            self.record_task_run_at(&task.id, status, now_epoch_seconds)?;
            if let Some(updated) = self.find_task(&task.id)? {
                ran.push(updated);
            }
        }

        Ok(ran)
    }

    async fn run_task_operation(&self, task: &Task) -> AppResult<TaskRunStatus> {
        if task.action != "Copy" {
            return Err(AppError::Validation(
                "only Copy tasks can be run manually".to_string(),
            ));
        }

        if should_skip_missing_session_backup_source(task)? {
            return Ok(TaskRunStatus::Skipped);
        }

        match (task.source_type.as_str(), task.target_type.as_str()) {
            ("Local", "Cloud") => {
                let settings = self.valid_webdav_settings()?;
                let file_states = {
                    let conn = self.db.connection()?;
                    FileState::load(&conn, &task.id)?
                };
                self.transfer
                    .push_local_to_cloud(task, &settings, &file_states)
                    .await?;
                let source = resolve_local_path(&task.source)?;
                let conn = self.db.connection()?;
                FileState::record(&conn, &task.id, &source)?;
                Ok(TaskRunStatus::Ok)
            }
            ("Cloud", "Local") => {
                let settings = self.valid_webdav_settings()?;
                let file_states = {
                    let conn = self.db.connection()?;
                    FileState::load(&conn, &task.id)?
                };
                self.transfer
                    .pull_cloud_to_local(task, &settings, &file_states)
                    .await?;
                let source = resolve_local_path(&task.target)?;
                let conn = self.db.connection()?;
                FileState::record(&conn, &task.id, &source)?;
                Ok(TaskRunStatus::Ok)
            }
            ("Local", "Local") => {
                let file_states = {
                    let conn = self.db.connection()?;
                    FileState::load(&conn, &task.id)?
                };
                copy_local_to_local(task, &file_states)?;
                let source = resolve_local_path(&task.source)?;
                let conn = self.db.connection()?;
                FileState::record(&conn, &task.id, &source)?;
                Ok(TaskRunStatus::Ok)
            }
            _ => Err(AppError::Validation(
                "Cloud to Cloud copy is not supported".to_string(),
            )),
        }
    }

    fn valid_webdav_settings(&self) -> AppResult<WebdavSettings> {
        let conn = self.db.connection()?;
        let settings = read_webdav_settings(&conn)?;
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
                last_run_at,
                COALESCE(last_status, 'never') AS status
            FROM tasks
            WHERE id = ?1
            "#,
            [id],
            task_from_row,
        )
        .optional()
        .map_err(Into::into)
        .map(|opt| {
            opt.map(|mut task| {
                task.link_state = derive_link_state(&task.action, &task.target_type, &task.target);
                task
            })
        })
    }

    fn record_task_run(&self, id: &str, status: &str) -> AppResult<()> {
        self.record_task_run_at(id, status, now_epoch_seconds()?)
    }

    fn record_task_run_at(&self, id: &str, status: &str, now: i64) -> AppResult<()> {
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
}

fn prepare_task(task: &CreateTaskInput) -> AppResult<PreparedTask> {
    let action = validate_one_of(
        &task.action,
        &["Symlink", "Junction", "Copy"],
        "task action",
    )?;
    let source_type = validate_one_of(&task.source_type, &["Local", "Cloud"], "source type")?;
    let target_type = validate_one_of(&task.target_type, &["Local", "Cloud"], "target type")?;
    if source_type == "Cloud" && target_type == "Cloud" {
        return Err(AppError::Validation(
            "Cloud to Cloud sync tasks are not supported".to_string(),
        ));
    }
    let source = normalize_task_path(&task.source, "task source")?;
    let target = normalize_task_path(&task.target, "task target")?;
    if (action == "Symlink" || action == "Junction")
        && (source_type != "Local" || target_type != "Local")
    {
        return Err(AppError::Validation(
            "symlink and junction tasks require local source and target".to_string(),
        ));
    }
    if action == "Junction" && !cfg!(target_os = "windows") {
        return Err(AppError::Validation(
            "Junction links are only supported on Windows".to_string(),
        ));
    }
    let schedule = normalize_task_schedule(&task.schedule, action)?;
    let direction = derive_direction(source_type, target_type);

    Ok(PreparedTask {
        action: action.to_string(),
        source_type: source_type.to_string(),
        source: source.to_string(),
        target_type: target_type.to_string(),
        target: target.to_string(),
        schedule,
        direction: direction.to_string(),
    })
}

fn normalize_task_path(raw: &str, label: &str) -> AppResult<String> {
    Ok(required_trimmed(raw, label)?.replace('\\', "/"))
}

async fn push_local_to_cloud(
    task: &Task,
    settings: &WebdavSettings,
    file_states: &FileStateMap,
    request_logger: &OutboundRequestLogger,
) -> AppResult<()> {
    let source = resolve_local_path(&task.source)?;
    let auth = webdav::auth_from_credentials(&settings.user, &settings.pass);

    if source.is_file() {
        push_local_file_to_cloud(
            &source,
            &task.target,
            settings,
            &auth,
            file_states,
            request_logger,
        )
        .await
    } else if source.is_dir() {
        push_local_directory_to_cloud(
            &source,
            &task.target,
            settings,
            &auth,
            file_states,
            request_logger,
        )
        .await
    } else {
        Err(AppError::Validation(format!(
            "local source does not exist: {}",
            task.source
        )))
    }
}

async fn pull_cloud_to_local(
    task: &Task,
    settings: &WebdavSettings,
    _file_states: &FileStateMap,
    request_logger: &OutboundRequestLogger,
) -> AppResult<()> {
    let auth = webdav::auth_from_credentials(&settings.user, &settings.pass);

    if task.source.ends_with('/') {
        let source_segments = remote_segments(settings, &task.source)?;
        let entries =
            webdav::list_directory(&settings.url, &source_segments, &auth, request_logger).await?;
        let target_root = resolve_local_path(&task.target)?;
        fs::create_dir_all(&target_root)?;
        for entry in entries {
            let mut child_segments = source_segments.clone();
            child_segments.push(entry.name.clone());
            let child_target = target_root.join(&entry.name);
            if entry.is_collection {
                fs::create_dir_all(&child_target)?;
                let child_task = Task {
                    source: task.source.clone() + &entry.name + "/",
                    target: child_target.to_string_lossy().into_owned(),
                    ..task.clone()
                };
                Box::pin(pull_cloud_to_local(
                    &child_task,
                    settings,
                    _file_states,
                    request_logger,
                ))
                .await?;
            } else {
                let bytes =
                    webdav::get_bytes(&settings.url, &child_segments, &auth, request_logger)
                        .await?;
                fs::write(&child_target, bytes)?;
            }
        }
        Ok(())
    } else {
        let source_segments = remote_segments(settings, &task.source)?;
        let bytes =
            webdav::get_bytes(&settings.url, &source_segments, &auth, request_logger).await?;
        let target = resolve_local_path(&task.target)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, bytes)?;
        Ok(())
    }
}

async fn push_local_file_to_cloud(
    source: &Path,
    target: &str,
    settings: &WebdavSettings,
    auth: &webdav::WebdavAuth,
    file_states: &FileStateMap,
    request_logger: &OutboundRequestLogger,
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

    let rel_path = required_file_name(source)?;
    if FileState::should_skip(source, &rel_path, file_states)? {
        return Ok(());
    }

    let parent_segments = file_segments[..file_segments.len() - 1].to_vec();
    webdav::ensure_remote_directories(&settings.url, &parent_segments, auth, request_logger)
        .await?;
    let url = webdav::build_remote_url(&settings.url, &file_segments)?;
    webdav::put_bytes(
        &url,
        auth,
        fs::read(source)?,
        "application/octet-stream",
        request_logger,
    )
    .await
}

async fn push_local_directory_to_cloud(
    source: &Path,
    target: &str,
    settings: &WebdavSettings,
    auth: &webdav::WebdavAuth,
    file_states: &FileStateMap,
    request_logger: &OutboundRequestLogger,
) -> AppResult<()> {
    let target_segments = remote_segments(settings, target)?;
    let mut directories = Vec::new();
    let mut files = Vec::new();
    collect_local_directory_push(
        source,
        target_segments,
        &mut directories,
        &mut files,
        file_states,
        source,
    )?;

    for directory in directories {
        webdav::ensure_remote_directories(&settings.url, &directory, auth, request_logger).await?;
    }

    for (path, file_segments) in files {
        let url = webdav::build_remote_url(&settings.url, &file_segments)?;
        webdav::put_bytes(
            &url,
            auth,
            fs::read(path)?,
            "application/octet-stream",
            request_logger,
        )
        .await?;
    }

    Ok(())
}

fn collect_local_directory_push(
    source: &Path,
    target_segments: Vec<String>,
    directories: &mut Vec<Vec<String>>,
    files: &mut Vec<(PathBuf, Vec<String>)>,
    file_states: &FileStateMap,
    source_root: &Path,
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
            collect_local_directory_push(
                &path,
                child_segments,
                directories,
                files,
                file_states,
                source_root,
            )?;
        } else if metadata.is_file() {
            let rel_path = path
                .strip_prefix(source_root)
                .map_err(|_| {
                    AppError::Internal("failed to compute relative path for push".to_string())
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if FileState::should_skip(&path, &rel_path, file_states)? {
                continue;
            }
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

fn copy_local_to_local(task: &Task, file_states: &FileStateMap) -> AppResult<()> {
    let source = resolve_local_path(&task.source)?;
    let target = resolve_local_path(&task.target)?;

    if !source.exists() {
        return Err(AppError::Validation(format!(
            "local source does not exist: {}",
            task.source
        )));
    }

    if fs::metadata(&source)?.is_file() {
        let rel_path = required_file_name(&source)?;
        if !FileState::should_skip(&source, &rel_path, file_states)? {
            copy_local_file(&source, &target)?;
        }
    } else {
        copy_local_directory(&source, &target, file_states)?;
    }
    Ok(())
}

fn should_skip_missing_session_backup_source(task: &Task) -> AppResult<bool> {
    if !task.id.starts_with("session-backup:") || task.source_type != "Local" {
        return Ok(false);
    }

    let source = resolve_local_path(&task.source)?;
    Ok(!source.exists())
}

fn copy_local_directory(source: &Path, target: &Path, file_states: &FileStateMap) -> AppResult<()> {
    let effective_target = if target.exists() && target.is_dir() {
        let name = required_file_name(source)?;
        target.join(name)
    } else {
        target.to_path_buf()
    };
    copy_directory_tree(source, &effective_target, file_states, source)
}

fn copy_local_file(source: &Path, target: &Path) -> AppResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)?;
    Ok(())
}

fn copy_directory_tree(
    source: &Path,
    target: &Path,
    file_states: &FileStateMap,
    source_root: &Path,
) -> AppResult<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let dest = target.join(entry.file_name());
        let metadata = fs::metadata(&path)?;
        if metadata.is_dir() {
            copy_directory_tree(&path, &dest, file_states, source_root)?;
        } else {
            let rel_path = path
                .strip_prefix(source_root)
                .map_err(|_| {
                    AppError::Internal("failed to compute relative path for copy".to_string())
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if !FileState::should_skip(&path, &rel_path, file_states)? {
                copy_local_file(&path, &dest)?;
            }
        }
    }
    Ok(())
}

fn required_file_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path file name must be valid UTF-8".to_string()))
}

fn create_link_placements(tasks: &[PreparedTask]) -> AppResult<Vec<PathBuf>> {
    let mut created = Vec::new();

    for task in tasks {
        match create_single_link_placement(task) {
            Ok(Some(path)) => created.push(path),
            Ok(None) => {}
            Err(error) => {
                remove_created_symlinks(&created);
                return Err(error);
            }
        }
    }

    Ok(created)
}

fn create_single_link_placement(task: &PreparedTask) -> AppResult<Option<PathBuf>> {
    create_task_link_placement(&task.action, &task.source, &task.target)
}

fn remove_created_symlinks(paths: &[PathBuf]) {
    remove_created_task_link_placements(paths);
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
            last_run_at,
            COALESCE(last_status, 'never') AS status
        FROM tasks
        WHERE group_id = ?1
        ORDER BY sort_index IS NULL, sort_index, created_at
        "#,
    )?;

    let rows = stmt.query_map([group_id], task_from_row)?;
    let mut tasks: Vec<Task> = rows.collect::<Result<Vec<_>, _>>()?;
    for task in &mut tasks {
        task.link_state = derive_link_state(&task.action, &task.target_type, &task.target);
    }
    Ok(tasks)
}

/// Trim a caller-supplied id ordering and reject empty or duplicate ids. `label` names the
/// entity for error messages (e.g. "task" → "task order contains duplicate id: …").
fn normalize_order(ids: Vec<String>, label: &str) -> AppResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(ids.len());
    let mut seen = HashSet::with_capacity(ids.len());
    for id in ids {
        let id = id.trim().to_string();
        if id.is_empty() {
            return Err(AppError::Validation(format!(
                "{label} order contains an empty id"
            )));
        }
        if !seen.insert(id.clone()) {
            return Err(AppError::Validation(format!(
                "{label} order contains duplicate id: {id}"
            )));
        }
        normalized.push(id);
    }
    Ok(normalized)
}

fn list_scheduled_copy_tasks(conn: &rusqlite::Connection) -> AppResult<Vec<Task>> {
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
            last_run_at,
            COALESCE(last_status, 'never') AS status
        FROM tasks
        WHERE action = 'Copy' AND schedule <> 'manual'
        ORDER BY sort_index IS NULL, sort_index, created_at
        "#,
    )?;

    let rows = stmt.query_map([], task_from_row)?;
    let mut tasks: Vec<Task> = rows.collect::<Result<Vec<_>, _>>()?;
    for task in &mut tasks {
        task.link_state = derive_link_state(&task.action, &task.target_type, &task.target);
    }
    Ok(tasks)
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
        last_run_at: row.get(8)?,
        status: row.get(9)?,
        link_state: String::new(),
    })
}

/// Placement health for a task's target. Link actions (Symlink / Junction) with a
/// Local target own a placement on disk; when that placement is missing the state is
/// `"missing"`. Copy tasks and Cloud targets have no link placement, so they report
/// `"present"` — there is nothing to go missing.
fn derive_link_state(action: &str, target_type: &str, target: &str) -> String {
    task_link_state(action, target_type, target).to_string()
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

fn validate_one_of<'a>(value: &'a str, allowed: &[&str], label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(AppError::Validation(format!("invalid {label}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingTransfer {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl Transfer for RecordingTransfer {
        fn push_local_to_cloud<'a>(
            &'a self,
            task: &'a Task,
            settings: &'a WebdavSettings,
            _file_states: &'a FileStateMap,
        ) -> TransferFuture<'a> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("lock transfer calls")
                    .push((task.source.clone(), settings.remote_root.clone()));
                Ok(())
            })
        }

        fn pull_cloud_to_local<'a>(
            &'a self,
            task: &'a Task,
            settings: &'a WebdavSettings,
            _file_states: &'a FileStateMap,
        ) -> TransferFuture<'a> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("lock transfer calls")
                    .push((task.target.clone(), settings.remote_root.clone()));
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn local_to_cloud_copy_runs_through_transfer_seam() {
        let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
        {
            let conn = db.connection().expect("open db connection");
            conn.execute(
                "UPDATE settings SET value = 'https://dav.example.com/root/' WHERE key = 'webdav_url'",
                [],
            )
            .expect("set webdav url");
            conn.execute(
                "UPDATE settings SET value = 'agent-nexus-sync' WHERE key = 'webdav_remote_root'",
                [],
            )
            .expect("set webdav remote root");
        }
        let transfer = Arc::new(RecordingTransfer::default());
        let request_logger = OutboundRequestLogger::for_test().expect("create request logger");
        let lifecycle = TaskLifecycle {
            db,
            request_logger,
            transfer: transfer.clone(),
        };
        let task = Task {
            id: "task-1".to_string(),
            direction: "Push".to_string(),
            action: "Copy".to_string(),
            source_type: "Local".to_string(),
            source: "~/source.txt".to_string(),
            target_type: "Cloud".to_string(),
            target: "backup/source.txt".to_string(),
            schedule: "manual".to_string(),
            last_run_at: None,
            status: "never".to_string(),
            link_state: "present".to_string(),
        };

        lifecycle
            .run_task_operation(&task)
            .await
            .expect("run through transfer seam");

        assert_eq!(
            *transfer.calls.lock().expect("lock transfer calls"),
            vec![("~/source.txt".to_string(), "agent-nexus-sync".to_string())],
        );
    }

    #[tokio::test]
    async fn cloud_to_local_copy_runs_through_transfer_seam() {
        let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
        {
            let conn = db.connection().expect("open db connection");
            conn.execute(
                "UPDATE settings SET value = 'https://dav.example.com/root/' WHERE key = 'webdav_url'",
                [],
            )
            .expect("set webdav url");
            conn.execute(
                "UPDATE settings SET value = 'agent-nexus-sync' WHERE key = 'webdav_remote_root'",
                [],
            )
            .expect("set webdav remote root");
        }
        let transfer = Arc::new(RecordingTransfer::default());
        let request_logger = OutboundRequestLogger::for_test().expect("create request logger");
        let lifecycle = TaskLifecycle {
            db,
            request_logger,
            transfer: transfer.clone(),
        };
        let task = Task {
            id: "task-2".to_string(),
            direction: "Pull".to_string(),
            action: "Copy".to_string(),
            source_type: "Cloud".to_string(),
            source: "backup/source.txt".to_string(),
            target_type: "Local".to_string(),
            target: "~/restored.txt".to_string(),
            schedule: "manual".to_string(),
            last_run_at: None,
            status: "never".to_string(),
            link_state: "present".to_string(),
        };

        lifecycle
            .run_task_operation(&task)
            .await
            .expect("run through transfer seam");

        assert_eq!(
            *transfer.calls.lock().expect("lock transfer calls"),
            vec![("~/restored.txt".to_string(), "agent-nexus-sync".to_string())],
        );
    }
}
