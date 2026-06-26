use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{
        app_config::AppConfigService,
        cron::{next_cron_occurrence_after_local, validate_cron_schedule},
        provider_quota::{
            http_client, is_token_expiring_soon, ClaudeCodeCredentials, HttpUsageTransport,
            LocalCredentialSource, CLAUDE_CODE_PROVIDER_ID,
        },
        util::required_trimmed,
    },
};

const DEFAULT_QUOTA_REFRESH_MINUTES: i64 = 5;
const WINDOW_ALIGN_PROMPT: &str = "hi";
const WINDOW_ALIGN_RETRY_SECONDS: i64 = 5 * 60;
const PROVIDER_WINDOW_SECONDS: i64 = 5 * 60 * 60;
const CLAUDE_CODE_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_CODE_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const CLAUDE_CODE_ALIAS: &str = "claude-code";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScheduleSettings {
    pub quota_refresh_minutes: i64,
    pub window_align_cron: String,
    pub window_align_model_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTriggerCapability {
    pub supported: bool,
    pub models: Vec<ProviderTriggerModel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTriggerModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProviderTriggerOutcome {
    prompt_tokens: i64,
    completion_tokens: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ProviderTriggerError {
    Retryable(String),
    Terminal(String),
}

impl ProviderTriggerError {
    fn message(&self) -> &str {
        match self {
            Self::Retryable(message) | Self::Terminal(message) => message,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

type ProviderTriggerFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProviderTriggerOutcome, ProviderTriggerError>> + Send + 'a>>;
type ProviderModelsFuture<'a> = Pin<
    Box<dyn Future<Output = Result<Vec<ProviderTriggerModel>, ProviderTriggerError>> + Send + 'a>,
>;

trait ProviderTriggerRunner: Send + Sync {
    fn supports(&self, provider_id: &str) -> bool;
    fn list_models<'a>(&'a self, provider_id: &'a str) -> ProviderModelsFuture<'a>;
    fn trigger<'a>(&'a self, provider_id: &'a str, model_id: &'a str) -> ProviderTriggerFuture<'a>;
}

#[derive(Clone)]
pub struct ProviderTriggerService {
    db: Arc<Database>,
    runner: Arc<dyn ProviderTriggerRunner>,
}

impl ProviderTriggerService {
    pub fn new(db: Arc<Database>, app_config: AppConfigService) -> Self {
        Self {
            db,
            runner: Arc::new(ClaudeCodeTriggerRunner::new(app_config)),
        }
    }

    #[cfg(test)]
    fn with_runner(db: Arc<Database>, runner: Arc<dyn ProviderTriggerRunner>) -> Self {
        Self { db, runner }
    }

    pub fn get_provider_schedule_settings(
        &self,
        provider_id: &str,
    ) -> AppResult<ProviderScheduleSettings> {
        let provider_id = normalize_provider_id(provider_id)?;
        let conn = self.db.connection()?;
        read_provider_schedule_settings(&conn, provider_id)
    }

    pub fn set_provider_schedule_settings(
        &self,
        provider_id: &str,
        settings: ProviderScheduleSettings,
    ) -> AppResult<ProviderScheduleSettings> {
        let provider_id = normalize_provider_id(provider_id)?;
        let settings = normalize_schedule_settings(settings)?;
        let active = is_window_alignment_active(&settings);
        if active && !self.runner.supports(provider_id) {
            return Err(AppError::Validation(format!(
                "window alignment is not supported for provider: {provider_id}"
            )));
        }

        let now = current_epoch_seconds();
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;
        let existing = read_provider_schedule_row(&tx, provider_id)?;
        let runtime = next_runtime_for_save(&settings, existing.as_ref(), now)?;

        tx.execute(
            "INSERT INTO provider_schedule_settings (
                provider_id,
                quota_refresh_minutes,
                window_align_cron,
                window_align_model_id,
                window_align_anchor_at,
                window_align_next_attempt_at,
                window_align_last_attempt_at,
                window_align_last_success_at,
                window_align_last_status,
                window_align_last_error,
                window_align_failure_count,
                created_at,
                updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12
            )
            ON CONFLICT(provider_id) DO UPDATE SET
                quota_refresh_minutes = excluded.quota_refresh_minutes,
                window_align_cron = excluded.window_align_cron,
                window_align_model_id = excluded.window_align_model_id,
                window_align_anchor_at = excluded.window_align_anchor_at,
                window_align_next_attempt_at = excluded.window_align_next_attempt_at,
                window_align_last_attempt_at = excluded.window_align_last_attempt_at,
                window_align_last_success_at = excluded.window_align_last_success_at,
                window_align_last_status = excluded.window_align_last_status,
                window_align_last_error = excluded.window_align_last_error,
                window_align_failure_count = excluded.window_align_failure_count,
                updated_at = excluded.updated_at",
            params![
                provider_id,
                settings.quota_refresh_minutes,
                settings.window_align_cron,
                settings.window_align_model_id,
                runtime.anchor_at,
                runtime.next_attempt_at,
                runtime.last_attempt_at,
                runtime.last_success_at,
                runtime.last_status,
                runtime.last_error,
                runtime.failure_count,
                now,
            ],
        )?;
        tx.commit()?;

        Ok(settings)
    }

    pub async fn list_provider_trigger_models(
        &self,
        provider_id: &str,
    ) -> AppResult<ProviderTriggerCapability> {
        let provider_id = normalize_provider_id(provider_id)?;
        if !self.runner.supports(provider_id) {
            return Ok(ProviderTriggerCapability {
                supported: false,
                models: Vec::new(),
            });
        }

        let models = self
            .runner
            .list_models(provider_id)
            .await
            .map_err(trigger_error_to_app_error)?;
        Ok(ProviderTriggerCapability {
            supported: true,
            models,
        })
    }

    pub async fn run_due_window_alignment(&self, now_epoch_seconds: i64) -> AppResult<Vec<String>> {
        let due = {
            let conn = self.db.connection()?;
            list_due_window_alignment_rows(&conn, now_epoch_seconds)?
        };
        let mut triggered = Vec::new();

        for row in due {
            let Some(model_id) = row.settings.window_align_model_id.as_deref() else {
                continue;
            };

            if let Some(next_allowed_at) = row
                .runtime
                .last_success_at
                .and_then(|last_success_at| last_success_at.checked_add(PROVIDER_WINDOW_SECONDS))
                .filter(|next_allowed_at| *next_allowed_at > now_epoch_seconds)
            {
                self.update_next_attempt_at(&row.provider_id, next_allowed_at)?;
                continue;
            }

            let result = self.runner.trigger(&row.provider_id, model_id).await;
            match result {
                Ok(outcome) => {
                    let next_anchor = next_cron_occurrence_after_local(
                        &row.settings.window_align_cron,
                        now_epoch_seconds,
                    )?;
                    let next_attempt_at = now_epoch_seconds
                        .checked_add(PROVIDER_WINDOW_SECONDS)
                        .map(|cooldown_at| next_anchor.max(cooldown_at))
                        .unwrap_or(next_anchor);
                    self.record_success(
                        &row.provider_id,
                        now_epoch_seconds,
                        next_anchor,
                        next_attempt_at,
                        outcome,
                    )?;
                    triggered.push(row.provider_id);
                }
                Err(error) if error.is_retryable() => {
                    let next_attempt_at = now_epoch_seconds + WINDOW_ALIGN_RETRY_SECONDS;
                    self.record_failure(
                        &row.provider_id,
                        now_epoch_seconds,
                        Some(next_attempt_at),
                        "retryable_failed",
                        error.message(),
                    )?;
                }
                Err(error) => {
                    self.record_failure(
                        &row.provider_id,
                        now_epoch_seconds,
                        None,
                        "terminal_failed",
                        error.message(),
                    )?;
                }
            }
        }

        Ok(triggered)
    }

    fn update_next_attempt_at(&self, provider_id: &str, next_attempt_at: i64) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "UPDATE provider_schedule_settings
             SET window_align_next_attempt_at = ?2, updated_at = ?3
             WHERE provider_id = ?1",
            params![provider_id, next_attempt_at, current_epoch_seconds()],
        )?;
        Ok(())
    }

    fn record_success(
        &self,
        provider_id: &str,
        now_epoch_seconds: i64,
        next_anchor: i64,
        next_attempt_at: i64,
        outcome: ProviderTriggerOutcome,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "UPDATE provider_schedule_settings
             SET window_align_anchor_at = ?2,
                 window_align_next_attempt_at = ?3,
                 window_align_last_attempt_at = ?4,
                 window_align_last_success_at = ?4,
                 window_align_last_status = 'success',
                 window_align_last_error = NULL,
                 window_align_failure_count = 0,
                 updated_at = ?4
             WHERE provider_id = ?1",
            params![provider_id, next_anchor, next_attempt_at, now_epoch_seconds],
        )?;
        let _tokens = outcome.prompt_tokens + outcome.completion_tokens;
        Ok(())
    }

    fn record_failure(
        &self,
        provider_id: &str,
        now_epoch_seconds: i64,
        next_attempt_at: Option<i64>,
        status: &str,
        error: &str,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "UPDATE provider_schedule_settings
             SET window_align_next_attempt_at = ?2,
                 window_align_last_attempt_at = ?3,
                 window_align_last_status = ?4,
                 window_align_last_error = ?5,
                 window_align_failure_count = window_align_failure_count + 1,
                 updated_at = ?3
             WHERE provider_id = ?1",
            params![
                provider_id,
                next_attempt_at,
                now_epoch_seconds,
                status,
                error
            ],
        )?;
        Ok(())
    }
}

#[derive(Clone)]
struct ClaudeCodeTriggerRunner {
    app_config: AppConfigService,
    credential_source: Arc<LocalCredentialSource>,
    usage_transport: Arc<HttpUsageTransport>,
}

impl ClaudeCodeTriggerRunner {
    fn new(app_config: AppConfigService) -> Self {
        Self {
            app_config,
            credential_source: Arc::new(LocalCredentialSource),
            usage_transport: Arc::new(HttpUsageTransport),
        }
    }

    async fn claude_access_token(
        &self,
    ) -> Result<(ClaudeCodeCredentials, String), ProviderTriggerError> {
        let credentials = self
            .credential_source
            .claude_code_credentials(&self.app_config)
            .map_err(|error| ProviderTriggerError::Terminal(error.to_string()))?
            .ok_or_else(|| {
                ProviderTriggerError::Terminal("Claude Code credentials were not found".to_string())
            })?;

        if !credentials.scopes.is_empty()
            && !credentials
                .scopes
                .iter()
                .any(|scope| scope == "user:profile")
        {
            return Err(ProviderTriggerError::Terminal(
                "Claude OAuth token missing 'user:profile' scope. Run 'claude setup-token'."
                    .to_string(),
            ));
        }

        let mut access_token = credentials.access_token.clone();
        if is_token_expiring_soon(credentials.expires_at) {
            if let Some(refreshed) = self
                .usage_transport
                .refresh_claude_code_credentials(&credentials)
                .await
            {
                access_token = refreshed;
            }
        }

        Ok((credentials, access_token))
    }

    async fn refresh_or_auth_error(
        &self,
        credentials: &ClaudeCodeCredentials,
    ) -> Result<String, ProviderTriggerError> {
        self.usage_transport
            .refresh_claude_code_credentials(credentials)
            .await
            .ok_or_else(|| {
                ProviderTriggerError::Terminal(
                    "Claude Code authorization was rejected; run claude /login".to_string(),
                )
            })
    }
}

impl ProviderTriggerRunner for ClaudeCodeTriggerRunner {
    fn supports(&self, provider_id: &str) -> bool {
        matches!(provider_id, CLAUDE_CODE_PROVIDER_ID | CLAUDE_CODE_ALIAS)
    }

    fn list_models<'a>(&'a self, provider_id: &'a str) -> ProviderModelsFuture<'a> {
        Box::pin(async move {
            if !self.supports(provider_id) {
                return Err(ProviderTriggerError::Terminal(format!(
                    "window alignment is not supported for provider: {provider_id}"
                )));
            }
            let (credentials, access_token) = self.claude_access_token().await?;
            match fetch_claude_code_models(&access_token).await {
                Err(ProviderTriggerError::Terminal(message))
                    if message.contains("authorization") =>
                {
                    let refreshed = self.refresh_or_auth_error(&credentials).await?;
                    fetch_claude_code_models(&refreshed).await
                }
                result => result,
            }
        })
    }

    fn trigger<'a>(&'a self, provider_id: &'a str, model_id: &'a str) -> ProviderTriggerFuture<'a> {
        Box::pin(async move {
            if !self.supports(provider_id) {
                return Err(ProviderTriggerError::Terminal(format!(
                    "window alignment is not supported for provider: {provider_id}"
                )));
            }
            let (credentials, access_token) = self.claude_access_token().await?;
            match trigger_claude_code(&access_token, model_id).await {
                Err(ProviderTriggerError::Terminal(message))
                    if message.contains("authorization") =>
                {
                    let refreshed = self.refresh_or_auth_error(&credentials).await?;
                    trigger_claude_code(&refreshed, model_id).await
                }
                result => result,
            }
        })
    }
}

#[derive(Clone, Debug)]
struct ProviderScheduleRow {
    provider_id: String,
    settings: ProviderScheduleSettings,
    runtime: WindowAlignmentRuntime,
}

#[derive(Clone, Debug)]
struct WindowAlignmentRuntime {
    anchor_at: Option<i64>,
    next_attempt_at: Option<i64>,
    last_attempt_at: Option<i64>,
    last_success_at: Option<i64>,
    last_status: String,
    last_error: Option<String>,
    failure_count: i64,
}

fn read_provider_schedule_settings(
    conn: &rusqlite::Connection,
    provider_id: &str,
) -> AppResult<ProviderScheduleSettings> {
    Ok(read_provider_schedule_row(conn, provider_id)?
        .map(|row| row.settings)
        .unwrap_or_else(default_schedule_settings))
}

fn read_provider_schedule_row(
    conn: &rusqlite::Connection,
    provider_id: &str,
) -> AppResult<Option<ProviderScheduleRow>> {
    conn.query_row(
        "SELECT
            provider_id,
            quota_refresh_minutes,
            window_align_cron,
            window_align_model_id,
            window_align_anchor_at,
            window_align_next_attempt_at,
            window_align_last_attempt_at,
            window_align_last_success_at,
            window_align_last_status,
            window_align_last_error,
            window_align_failure_count
         FROM provider_schedule_settings
         WHERE provider_id = ?1",
        [provider_id],
        provider_schedule_row_from_sql,
    )
    .optional()
    .map_err(Into::into)
}

fn list_due_window_alignment_rows(
    conn: &rusqlite::Connection,
    now_epoch_seconds: i64,
) -> AppResult<Vec<ProviderScheduleRow>> {
    let mut stmt = conn.prepare(
        "SELECT
            provider_id,
            quota_refresh_minutes,
            window_align_cron,
            window_align_model_id,
            window_align_anchor_at,
            window_align_next_attempt_at,
            window_align_last_attempt_at,
            window_align_last_success_at,
            window_align_last_status,
            window_align_last_error,
            window_align_failure_count
         FROM provider_schedule_settings
         WHERE TRIM(window_align_cron) <> ''
           AND COALESCE(TRIM(window_align_model_id), '') <> ''
           AND window_align_next_attempt_at IS NOT NULL
           AND window_align_next_attempt_at <= ?1
         ORDER BY window_align_next_attempt_at, provider_id",
    )?;
    let rows = stmt.query_map([now_epoch_seconds], provider_schedule_row_from_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn provider_schedule_row_from_sql(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderScheduleRow> {
    Ok(ProviderScheduleRow {
        provider_id: row.get(0)?,
        settings: ProviderScheduleSettings {
            quota_refresh_minutes: row.get(1)?,
            window_align_cron: row.get(2)?,
            window_align_model_id: row.get(3)?,
        },
        runtime: WindowAlignmentRuntime {
            anchor_at: row.get(4)?,
            next_attempt_at: row.get(5)?,
            last_attempt_at: row.get(6)?,
            last_success_at: row.get(7)?,
            last_status: row.get(8)?,
            last_error: row.get(9)?,
            failure_count: row.get(10)?,
        },
    })
}

fn next_runtime_for_save(
    settings: &ProviderScheduleSettings,
    existing: Option<&ProviderScheduleRow>,
    now_epoch_seconds: i64,
) -> AppResult<WindowAlignmentRuntime> {
    let active = is_window_alignment_active(settings);
    if !active {
        return Ok(WindowAlignmentRuntime {
            anchor_at: None,
            next_attempt_at: None,
            last_attempt_at: None,
            last_success_at: existing.and_then(|row| row.runtime.last_success_at),
            last_status: "never".to_string(),
            last_error: None,
            failure_count: 0,
        });
    }

    if let Some(existing) = existing {
        if existing.settings.window_align_cron == settings.window_align_cron
            && existing.settings.window_align_model_id == settings.window_align_model_id
            && existing.runtime.next_attempt_at.is_some()
        {
            return Ok(existing.runtime.clone());
        }
    }

    let anchor_at =
        next_cron_occurrence_after_local(&settings.window_align_cron, now_epoch_seconds - 60)?;
    let last_success_at = existing.and_then(|row| row.runtime.last_success_at);
    let next_attempt_at = last_success_at
        .and_then(|last_success_at| last_success_at.checked_add(PROVIDER_WINDOW_SECONDS))
        .map(|cooldown_at| anchor_at.max(cooldown_at))
        .unwrap_or(anchor_at);

    Ok(WindowAlignmentRuntime {
        anchor_at: Some(anchor_at),
        next_attempt_at: Some(next_attempt_at),
        last_attempt_at: None,
        last_success_at,
        last_status: "never".to_string(),
        last_error: None,
        failure_count: 0,
    })
}

fn default_schedule_settings() -> ProviderScheduleSettings {
    ProviderScheduleSettings {
        quota_refresh_minutes: DEFAULT_QUOTA_REFRESH_MINUTES,
        window_align_cron: String::new(),
        window_align_model_id: None,
    }
}

fn normalize_schedule_settings(
    settings: ProviderScheduleSettings,
) -> AppResult<ProviderScheduleSettings> {
    if settings.quota_refresh_minutes < 1 {
        return Err(AppError::Validation(
            "quota refresh minutes must be at least 1".to_string(),
        ));
    }

    let window_align_cron = settings.window_align_cron.trim().to_string();
    if !window_align_cron.is_empty() {
        validate_cron_schedule(&window_align_cron)?;
    }

    Ok(ProviderScheduleSettings {
        quota_refresh_minutes: settings.quota_refresh_minutes,
        window_align_cron,
        window_align_model_id: settings.window_align_model_id.and_then(|model_id| {
            let model_id = model_id.trim().to_string();
            (!model_id.is_empty()).then_some(model_id)
        }),
    })
}

fn is_window_alignment_active(settings: &ProviderScheduleSettings) -> bool {
    !settings.window_align_cron.trim().is_empty()
        && settings
            .window_align_model_id
            .as_deref()
            .is_some_and(|model_id| !model_id.trim().is_empty())
}

fn normalize_provider_id(provider_id: &str) -> AppResult<&str> {
    required_trimmed(provider_id, "provider id")
}

fn trigger_error_to_app_error(error: ProviderTriggerError) -> AppError {
    AppError::Validation(error.message().to_string())
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Debug, Deserialize)]
struct ClaudeModelsResponse {
    #[serde(default)]
    data: Vec<ClaudeModel>,
}

#[derive(Debug, Deserialize)]
struct ClaudeModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessageResponse {
    usage: Option<ClaudeMessageUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessageUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

async fn fetch_claude_code_models(
    access_token: &str,
) -> Result<Vec<ProviderTriggerModel>, ProviderTriggerError> {
    let response = http_client()
        .get(CLAUDE_CODE_MODELS_URL)
        .bearer_auth(access_token)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    ensure_success_status_or_trigger_error(status, &body, "Claude Code models")?;
    let parsed = serde_json::from_str::<ClaudeModelsResponse>(&body)
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;

    Ok(parsed
        .data
        .into_iter()
        .map(|model| ProviderTriggerModel {
            display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
        })
        .collect())
}

async fn trigger_claude_code(
    access_token: &str,
    model_id: &str,
) -> Result<ProviderTriggerOutcome, ProviderTriggerError> {
    let response = http_client()
        .post(CLAUDE_CODE_MESSAGES_URL)
        .bearer_auth(access_token)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(
            serde_json::json!({
                "model": model_id,
                "max_tokens": 1,
                "stream": false,
                "messages": [{"role": "user", "content": WINDOW_ALIGN_PROMPT}]
            })
            .to_string(),
        )
        .send()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    ensure_success_status_or_trigger_error(status, &body, "Claude Code messages")?;
    let parsed = serde_json::from_str::<ClaudeMessageResponse>(&body)
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    Ok(ProviderTriggerOutcome {
        prompt_tokens: parsed
            .usage
            .as_ref()
            .and_then(|usage| usage.input_tokens)
            .unwrap_or(0),
        completion_tokens: parsed
            .usage
            .and_then(|usage| usage.output_tokens)
            .unwrap_or(0),
    })
}

fn ensure_success_status_or_trigger_error(
    status: reqwest::StatusCode,
    body: &str,
    label: &str,
) -> Result<(), ProviderTriggerError> {
    if status.is_success() {
        return Ok(());
    }
    let detail = response_error_detail(body);
    let message = if detail.is_empty() {
        format!("{label} endpoint returned {status}")
    } else {
        format!("{label} endpoint returned {status}: {detail}")
    };
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderTriggerError::Terminal(
            "Claude Code authorization was rejected; run claude /login".to_string(),
        ));
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return Err(ProviderTriggerError::Retryable(message));
    }
    Err(ProviderTriggerError::Terminal(message))
}

fn response_error_detail(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| trimmed.chars().take(240).collect())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use super::*;

    #[derive(Default)]
    struct FakeRunner {
        supported: bool,
        outcomes: Mutex<VecDeque<Result<ProviderTriggerOutcome, ProviderTriggerError>>>,
    }

    impl FakeRunner {
        fn supported_with(
            outcomes: Vec<Result<ProviderTriggerOutcome, ProviderTriggerError>>,
        ) -> Self {
            Self {
                supported: true,
                outcomes: Mutex::new(VecDeque::from(outcomes)),
            }
        }
    }

    impl ProviderTriggerRunner for FakeRunner {
        fn supports(&self, _provider_id: &str) -> bool {
            self.supported
        }

        fn list_models<'a>(&'a self, _provider_id: &'a str) -> ProviderModelsFuture<'a> {
            Box::pin(async {
                Ok(vec![ProviderTriggerModel {
                    id: "model-1".to_string(),
                    display_name: "Model 1".to_string(),
                }])
            })
        }

        fn trigger<'a>(
            &'a self,
            _provider_id: &'a str,
            _model_id: &'a str,
        ) -> ProviderTriggerFuture<'a> {
            Box::pin(async move {
                self.outcomes
                    .lock()
                    .expect("lock fake outcomes")
                    .pop_front()
                    .expect("fake outcome is queued")
            })
        }
    }

    fn test_service(runner: FakeRunner) -> ProviderTriggerService {
        let db = Arc::new(Database::open_in_memory().expect("open db"));
        ProviderTriggerService::with_runner(db, Arc::new(runner))
    }

    #[test]
    fn default_schedule_settings_are_inactive() {
        let service = test_service(FakeRunner::default());

        let settings = service
            .get_provider_schedule_settings("claude")
            .expect("read default settings");

        assert_eq!(
            settings,
            ProviderScheduleSettings {
                quota_refresh_minutes: DEFAULT_QUOTA_REFRESH_MINUTES,
                window_align_cron: String::new(),
                window_align_model_id: None,
            }
        );
    }

    #[test]
    fn set_schedule_rejects_invalid_cron_lists() {
        let service = test_service(FakeRunner {
            supported: true,
            ..Default::default()
        });

        let result = service.set_provider_schedule_settings(
            "claude",
            ProviderScheduleSettings {
                quota_refresh_minutes: 5,
                window_align_cron: "0 5,,10 * * *".to_string(),
                window_align_model_id: Some("model-1".to_string()),
            },
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retryable_failure_schedules_five_minute_retry() {
        let service = test_service(FakeRunner::supported_with(vec![Err(
            ProviderTriggerError::Retryable("temporary failure".to_string()),
        )]));
        service
            .set_provider_schedule_settings(
                "claude",
                ProviderScheduleSettings {
                    quota_refresh_minutes: 5,
                    window_align_cron: "* * * * *".to_string(),
                    window_align_model_id: Some("model-1".to_string()),
                },
            )
            .expect("save schedule");

        let now = current_epoch_seconds() + 60;
        service
            .run_due_window_alignment(now)
            .await
            .expect("run due trigger");

        let row = {
            let conn = service.db.connection().expect("db connection");
            read_provider_schedule_row(&conn, "claude")
                .expect("read row")
                .expect("row exists")
        };
        assert_eq!(row.runtime.last_status, "retryable_failed");
        assert_eq!(
            row.runtime.next_attempt_at,
            Some(now + WINDOW_ALIGN_RETRY_SECONDS)
        );
    }

    #[tokio::test]
    async fn terminal_failure_stops_automatic_retry() {
        let service = test_service(FakeRunner::supported_with(vec![Err(
            ProviderTriggerError::Terminal("auth expired".to_string()),
        )]));
        service
            .set_provider_schedule_settings(
                "claude",
                ProviderScheduleSettings {
                    quota_refresh_minutes: 5,
                    window_align_cron: "* * * * *".to_string(),
                    window_align_model_id: Some("model-1".to_string()),
                },
            )
            .expect("save schedule");

        let now = current_epoch_seconds() + 60;
        service
            .run_due_window_alignment(now)
            .await
            .expect("run due trigger");

        let row = {
            let conn = service.db.connection().expect("db connection");
            read_provider_schedule_row(&conn, "claude")
                .expect("read row")
                .expect("row exists")
        };
        assert_eq!(row.runtime.last_status, "terminal_failed");
        assert_eq!(row.runtime.next_attempt_at, None);
    }

    #[tokio::test]
    async fn success_delays_next_attempt_until_five_hour_cooldown() {
        let service = test_service(FakeRunner::supported_with(vec![Ok(
            ProviderTriggerOutcome {
                prompt_tokens: 1,
                completion_tokens: 1,
            },
        )]));
        service
            .set_provider_schedule_settings(
                "claude",
                ProviderScheduleSettings {
                    quota_refresh_minutes: 5,
                    window_align_cron: "* * * * *".to_string(),
                    window_align_model_id: Some("model-1".to_string()),
                },
            )
            .expect("save schedule");

        let now = current_epoch_seconds() + 60;
        service
            .run_due_window_alignment(now)
            .await
            .expect("run due trigger");

        let row = {
            let conn = service.db.connection().expect("db connection");
            read_provider_schedule_row(&conn, "claude")
                .expect("read row")
                .expect("row exists")
        };
        assert_eq!(row.runtime.last_status, "success");
        assert_eq!(
            row.runtime.next_attempt_at,
            Some(now + PROVIDER_WINDOW_SECONDS)
        );
    }
}
