use std::sync::Arc;

mod task_lifecycle;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{outbound_request_log::OutboundRequestLogger, util::required_trimmed, webdav},
};

use task_lifecycle::TaskLifecycle;

const WEBDAV_URL_SETTING: &str = "webdav_url";
const WEBDAV_USER_SETTING: &str = "webdav_user";
const WEBDAV_PASS_SETTING: &str = "webdav_pass";
const WEBDAV_REMOTE_ROOT_SETTING: &str = "webdav_remote_root";
const DEFAULT_WEBDAV_REMOTE_ROOT: &str = "agent-nexus-sync";
const SESSION_BACKUP_SOURCE_TEMPLATE: &str = "{{project_dir}}/__sessions/";
const SESSION_BACKUP_TARGET_TEMPLATE: &str = "Session/{{project_key}}/";
const SESSION_BACKUP_SCHEDULE: &str = "0 * * * *";
const SESSION_BACKUP_GROUP_ID: &str = "system:session-backup";
const SESSION_BACKUP_SYSTEM_KIND: &str = "session_backup";

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
    pub link_state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
    pub id: String,
    pub name: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBackup {
    pub project_key: String,
    pub task: Task,
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

#[derive(Clone)]
pub struct SyncService {
    db: Arc<Database>,
    task_lifecycle: TaskLifecycle,
}

impl SyncService {
    pub fn new(db: Arc<Database>, request_logger: OutboundRequestLogger) -> Self {
        Self {
            task_lifecycle: TaskLifecycle::new(db.clone(), request_logger.clone()),
            db,
        }
    }

    pub fn get_webdav_settings(&self) -> AppResult<WebdavSettings> {
        let conn = self.db.connection()?;
        read_webdav_settings(&conn)
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
        webdav::test_connection(&settings.url, &auth, self.task_lifecycle.request_logger()).await?;
        let segments = webdav::path_segments(&settings.remote_root)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        webdav::ensure_remote_directories(
            &settings.url,
            &segments,
            &auth,
            self.task_lifecycle.request_logger(),
        )
        .await
    }

    pub async fn run_task(&self, id: String) -> AppResult<Task> {
        self.task_lifecycle.run_task(id).await
    }

    pub async fn run_due_scheduled_tasks(&self, now_epoch_seconds: i64) -> AppResult<Vec<Task>> {
        self.task_lifecycle
            .run_due_scheduled_tasks(now_epoch_seconds)
            .await
    }

    pub fn list_task_groups(&self) -> AppResult<Vec<TaskGroup>> {
        self.task_lifecycle.list_task_groups()
    }

    pub fn list_session_backups(&self) -> AppResult<Vec<SessionBackup>> {
        self.task_lifecycle.list_session_backups()
    }

    pub fn create_task_group(&self, input: CreateTaskGroupInput) -> AppResult<TaskGroup> {
        self.task_lifecycle.create_task_group(input)
    }

    pub fn delete_task(&self, id: String) -> AppResult<()> {
        self.task_lifecycle.delete_task(id)
    }

    pub fn delete_task_group(&self, id: String) -> AppResult<()> {
        self.task_lifecycle.delete_task_group(id)
    }

    pub fn add_task(&self, group_id: String, task: CreateTaskInput) -> AppResult<TaskGroup> {
        self.task_lifecycle.add_task(group_id, task)
    }

    pub fn update_task_schedule(&self, id: String, schedule: String) -> AppResult<Task> {
        self.task_lifecycle.update_task_schedule(id, schedule)
    }
}

fn render_project_template(
    template: &str,
    project_dir: &str,
    project_key: &str,
) -> AppResult<String> {
    let mut rendered = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let variable_and_rest = &remaining[start + 2..];
        let end = variable_and_rest.find("}}").ok_or_else(|| {
            AppError::Validation("sync template variable is not closed".to_string())
        })?;
        let variable = &variable_and_rest[..end];
        rendered.push_str(match variable {
            "project_dir" => project_dir,
            "project_key" => project_key,
            _ => {
                return Err(AppError::Validation(format!(
                    "unknown sync template variable: {variable}"
                )))
            }
        });
        remaining = &variable_and_rest[end + 2..];
    }

    if remaining.contains("}}") {
        return Err(AppError::Validation(
            "sync template variable has no opening delimiter".to_string(),
        ));
    }
    rendered.push_str(remaining);
    Ok(rendered)
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

pub(crate) fn read_webdav_settings(conn: &rusqlite::Connection) -> AppResult<WebdavSettings> {
    Ok(WebdavSettings {
        url: read_setting(conn, WEBDAV_URL_SETTING)?.unwrap_or_default(),
        user: read_setting(conn, WEBDAV_USER_SETTING)?.unwrap_or_default(),
        pass: read_setting(conn, WEBDAV_PASS_SETTING)?.unwrap_or_default(),
        remote_root: read_setting(conn, WEBDAV_REMOTE_ROOT_SETTING)?
            .unwrap_or_else(|| DEFAULT_WEBDAV_REMOTE_ROOT.to_string()),
    })
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
