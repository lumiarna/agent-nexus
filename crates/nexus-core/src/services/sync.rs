use std::sync::Arc;

mod task_lifecycle;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::webdav,
};

use task_lifecycle::TaskLifecycle;

const WEBDAV_URL_SETTING: &str = "webdav_url";
const WEBDAV_USER_SETTING: &str = "webdav_user";
const WEBDAV_PASS_SETTING: &str = "webdav_pass";
const WEBDAV_REMOTE_ROOT_SETTING: &str = "webdav_remote_root";
const DEFAULT_WEBDAV_REMOTE_ROOT: &str = "agent-nexus-sync";

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
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            task_lifecycle: TaskLifecycle::new(db.clone()),
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
        webdav::test_connection(&settings.url, &auth).await?;
        let segments = webdav::path_segments(&settings.remote_root)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        webdav::ensure_remote_directories(&settings.url, &segments, &auth).await
    }

    pub async fn run_task(&self, id: String) -> AppResult<Task> {
        self.task_lifecycle.run_task(id).await
    }

    pub fn list_task_groups(&self) -> AppResult<Vec<TaskGroup>> {
        self.task_lifecycle.list_task_groups()
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

fn read_webdav_settings(conn: &rusqlite::Connection) -> AppResult<WebdavSettings> {
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

fn required_trimmed<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::Validation(format!("{label} is required")))
    } else {
        Ok(value)
    }
}
