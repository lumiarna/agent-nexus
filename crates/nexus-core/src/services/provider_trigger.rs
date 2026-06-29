use std::{
    collections::BTreeMap,
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, Date, OffsetDateTime};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{
        app_config::AppConfigService,
        cron::{next_cron_occurrence_after_local, validate_cron_schedule},
        outbound_request_log::{
            OutboundRequestContext, OutboundRequestError, OutboundRequestLogger,
        },
        provider_quota::{
            http_client, ClaudeAccessToken, ClaudeAuthError, HttpUsageTransport,
            LocalCredentialSource, CLAUDE_CODE_PROVIDER_ID,
        },
        util::required_trimmed,
    },
};

const DEFAULT_QUOTA_REFRESH_MINUTES: i64 = 5;
const WINDOW_ALIGN_PROMPT: &str = ".";
const WINDOW_ALIGN_RETRY_SECONDS: i64 = 5 * 60;
const PROVIDER_WINDOW_SECONDS: i64 = 5 * 60 * 60;
const CLAUDE_CODE_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_CODE_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const CLAUDE_CODE_ALIAS: &str = "claude-code";
const CLAUDE_UNIFIED_STATUS_HEADER: &str = "anthropic-ratelimit-unified-status";
const CLAUDE_UNIFIED_CLAIM_HEADER: &str = "anthropic-ratelimit-unified-representative-claim";
const CLAUDE_UNIFIED_RESET_HEADER: &str = "anthropic-ratelimit-unified-reset";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScheduleSettings {
    pub quota_refresh_minutes: i64,
    pub window_align_cron: String,
    pub window_align_model_id: Option<String>,
    /// Read-only: the next scheduled attempt time; recomputed by the backend on
    /// every save/run, so any value sent in on a write is ignored.
    #[serde(default)]
    pub window_align_next_attempt_at: Option<i64>,
    #[serde(default)]
    pub window_align_last_attempt_at: Option<i64>,
    #[serde(default = "default_window_align_last_status")]
    pub window_align_last_status: String,
    #[serde(default)]
    pub window_align_last_error: Option<String>,
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
    AuthRequired,
}

impl ProviderTriggerError {
    fn message(&self) -> &str {
        match self {
            Self::Retryable(message) | Self::Terminal(message) => message,
            Self::AuthRequired => "Claude Code authorization was rejected; run claude /login",
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl From<ClaudeAuthError> for ProviderTriggerError {
    fn from(error: ClaudeAuthError) -> Self {
        match error {
            ClaudeAuthError::NoCreds
            | ClaudeAuthError::MissingScope { .. }
            | ClaudeAuthError::RefreshFailed { .. }
            | ClaudeAuthError::RefreshRejected { .. }
            | ClaudeAuthError::Terminal(_) => ProviderTriggerError::Terminal(error.message()),
        }
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
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl ProviderTriggerService {
    pub fn new(
        db: Arc<Database>,
        app_config: AppConfigService,
        request_logger: OutboundRequestLogger,
    ) -> Self {
        Self {
            db,
            runner: Arc::new(ClaudeCodeTriggerRunner::new(app_config, request_logger)),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    #[cfg(test)]
    fn with_runner(db: Arc<Database>, runner: Arc<dyn ProviderTriggerRunner>) -> Self {
        Self {
            db,
            runner,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        }
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
        {
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
        }

        let conn = self.db.connection()?;
        read_provider_schedule_settings(&conn, provider_id)
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

            if let Some(last_success_at) = row.runtime.last_success_at {
                let next_allowed_at = next_window_alignment_attempt_after_success(
                    &row.settings.window_align_cron,
                    last_success_at,
                )?;
                if next_allowed_at > now_epoch_seconds {
                    self.update_next_attempt_at(&row.provider_id, next_allowed_at)?;
                    continue;
                }
            }

            let Some(_run_guard) = self.try_begin_window_alignment(&row.provider_id)? else {
                continue;
            };
            let result = self.runner.trigger(&row.provider_id, model_id).await;
            match result {
                Ok(outcome) => {
                    let next_anchor = next_cron_occurrence_after_local(
                        &row.settings.window_align_cron,
                        now_epoch_seconds,
                    )?;
                    let next_attempt_at = next_window_alignment_attempt_after_success(
                        &row.settings.window_align_cron,
                        now_epoch_seconds,
                    )?;
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

    pub async fn run_window_alignment_now(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> AppResult<ProviderScheduleSettings> {
        let provider_id = normalize_provider_id(provider_id)?;
        let model_id = required_trimmed(model_id, "window alignment model")?;
        if !self.runner.supports(provider_id) {
            return Err(AppError::Validation(format!(
                "window alignment is not supported for provider: {provider_id}"
            )));
        }

        self.ensure_provider_schedule_row(provider_id)?;
        let Some(_run_guard) = self.try_begin_window_alignment(provider_id)? else {
            let conn = self.db.connection()?;
            return read_provider_schedule_settings(&conn, provider_id);
        };
        let now = current_epoch_seconds();
        let result = self.runner.trigger(provider_id, model_id).await;
        match result {
            Ok(outcome) => self.record_manual_success(provider_id, now, outcome)?,
            Err(error) => self.record_manual_failure(
                provider_id,
                now,
                if error.is_retryable() {
                    "retryable_failed"
                } else {
                    "terminal_failed"
                },
                error.message(),
            )?,
        }

        let conn = self.db.connection()?;
        read_provider_schedule_settings(&conn, provider_id)
    }

    fn try_begin_window_alignment(
        &self,
        provider_id: &str,
    ) -> AppResult<Option<WindowAlignmentRunGuard>> {
        let mut in_flight = self
            .in_flight
            .lock()
            .map_err(|_| AppError::Internal("window alignment state lock poisoned".to_string()))?;
        if !in_flight.insert(provider_id.to_string()) {
            return Ok(None);
        }
        Ok(Some(WindowAlignmentRunGuard {
            provider_id: provider_id.to_string(),
            in_flight: Arc::clone(&self.in_flight),
        }))
    }

    fn ensure_provider_schedule_row(&self, provider_id: &str) -> AppResult<()> {
        let now = current_epoch_seconds();
        let settings = default_schedule_settings();
        let conn = self.db.connection()?;
        conn.execute(
            "INSERT INTO provider_schedule_settings (
                provider_id,
                quota_refresh_minutes,
                window_align_cron,
                window_align_model_id,
                window_align_last_status,
                window_align_failure_count,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, 'never', 0, ?5, ?5)
            ON CONFLICT(provider_id) DO NOTHING",
            params![
                provider_id,
                settings.quota_refresh_minutes,
                settings.window_align_cron,
                settings.window_align_model_id,
                now,
            ],
        )?;
        Ok(())
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

    fn record_manual_success(
        &self,
        provider_id: &str,
        now_epoch_seconds: i64,
        outcome: ProviderTriggerOutcome,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "UPDATE provider_schedule_settings
             SET window_align_last_attempt_at = ?2,
                 window_align_last_success_at = ?2,
                 window_align_last_status = 'success',
                 window_align_last_error = NULL,
                 window_align_failure_count = 0,
                 updated_at = ?2
             WHERE provider_id = ?1",
            params![provider_id, now_epoch_seconds],
        )?;
        let _tokens = outcome.prompt_tokens + outcome.completion_tokens;
        Ok(())
    }

    fn record_manual_failure(
        &self,
        provider_id: &str,
        now_epoch_seconds: i64,
        status: &str,
        error: &str,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "UPDATE provider_schedule_settings
             SET window_align_last_attempt_at = ?2,
                 window_align_last_status = ?3,
                 window_align_last_error = ?4,
                 window_align_failure_count = window_align_failure_count + 1,
                 updated_at = ?2
             WHERE provider_id = ?1",
            params![provider_id, now_epoch_seconds, status, error],
        )?;
        Ok(())
    }
}

struct WindowAlignmentRunGuard {
    provider_id: String,
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl Drop for WindowAlignmentRunGuard {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = self.in_flight.lock() {
            in_flight.remove(&self.provider_id);
        }
    }
}

#[derive(Clone)]
struct ClaudeCodeTriggerRunner {
    app_config: AppConfigService,
    credential_source: Arc<LocalCredentialSource>,
    usage_transport: Arc<HttpUsageTransport>,
    request_logger: OutboundRequestLogger,
}

impl ClaudeCodeTriggerRunner {
    fn new(app_config: AppConfigService, request_logger: OutboundRequestLogger) -> Self {
        Self {
            app_config,
            credential_source: Arc::new(LocalCredentialSource),
            usage_transport: Arc::new(HttpUsageTransport::new(request_logger.clone())),
            request_logger,
        }
    }

    fn auth(&self) -> ClaudeAccessToken<'_> {
        ClaudeAccessToken::new(
            &self.app_config,
            self.credential_source.as_ref(),
            self.usage_transport.as_ref(),
        )
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
            let auth = self.auth();
            let (credentials, access_token) = auth.acquire().await?;
            auth.with_auth_retry(
                &credentials,
                access_token,
                |access_token| async move {
                    fetch_claude_code_models(&access_token, &self.request_logger).await
                },
                |error| matches!(error, ProviderTriggerError::AuthRequired),
            )
            .await
        })
    }

    fn trigger<'a>(&'a self, provider_id: &'a str, model_id: &'a str) -> ProviderTriggerFuture<'a> {
        Box::pin(async move {
            if !self.supports(provider_id) {
                return Err(ProviderTriggerError::Terminal(format!(
                    "window alignment is not supported for provider: {provider_id}"
                )));
            }
            let auth = self.auth();
            let (credentials, access_token) = auth.acquire().await?;
            auth.with_auth_retry(
                &credentials,
                access_token,
                |access_token| async move {
                    trigger_claude_code(&access_token, model_id, &self.request_logger).await
                },
                |error| matches!(error, ProviderTriggerError::AuthRequired),
            )
            .await
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
    let last_attempt_at: Option<i64> = row.get(6)?;
    let last_status: String = row.get(8)?;
    let last_error: Option<String> = row.get(9)?;
    Ok(ProviderScheduleRow {
        provider_id: row.get(0)?,
        settings: ProviderScheduleSettings {
            quota_refresh_minutes: row.get(1)?,
            window_align_cron: row.get(2)?,
            window_align_model_id: row.get(3)?,
            window_align_next_attempt_at: row.get(5)?,
            window_align_last_attempt_at: last_attempt_at,
            window_align_last_status: last_status.clone(),
            window_align_last_error: last_error.clone(),
        },
        runtime: WindowAlignmentRuntime {
            anchor_at: row.get(4)?,
            next_attempt_at: row.get(5)?,
            last_attempt_at,
            last_success_at: row.get(7)?,
            last_status,
            last_error,
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
    let next_attempt_at = match last_success_at {
        Some(last_success_at) => {
            let success_based = next_window_alignment_attempt_after_success(
                &settings.window_align_cron,
                last_success_at,
            )?;
            if success_based > now_epoch_seconds {
                success_based
            } else {
                anchor_at
            }
        }
        None => anchor_at,
    };

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
        window_align_next_attempt_at: None,
        window_align_last_attempt_at: None,
        window_align_last_status: default_window_align_last_status(),
        window_align_last_error: None,
    }
}

fn default_window_align_last_status() -> String {
    "never".to_string()
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
        validate_daily_window_alignment_schedule(&window_align_cron)?;
    }

    Ok(ProviderScheduleSettings {
        quota_refresh_minutes: settings.quota_refresh_minutes,
        window_align_cron,
        window_align_model_id: settings.window_align_model_id.and_then(|model_id| {
            let model_id = model_id.trim().to_string();
            (!model_id.is_empty()).then_some(model_id)
        }),
        window_align_next_attempt_at: settings.window_align_next_attempt_at,
        window_align_last_attempt_at: settings.window_align_last_attempt_at,
        window_align_last_status: settings.window_align_last_status,
        window_align_last_error: settings.window_align_last_error,
    })
}

fn is_window_alignment_active(settings: &ProviderScheduleSettings) -> bool {
    !settings.window_align_cron.trim().is_empty()
        && settings
            .window_align_model_id
            .as_deref()
            .is_some_and(|model_id| !model_id.trim().is_empty())
}

fn next_window_alignment_attempt_after_success(
    schedule: &str,
    last_success_at: i64,
) -> AppResult<i64> {
    let cooldown_at = last_success_at
        .checked_add(PROVIDER_WINDOW_SECONDS)
        .ok_or_else(|| AppError::Validation("window alignment time overflow".to_string()))?;
    if local_date(cooldown_at)? > local_date(last_success_at)? {
        let next_daily_start = next_cron_occurrence_after_local(schedule, last_success_at)?;
        Ok(next_daily_start.max(cooldown_at))
    } else {
        Ok(cooldown_at)
    }
}

fn validate_daily_window_alignment_schedule(schedule: &str) -> AppResult<()> {
    validate_cron_schedule(schedule)?;
    let fields = schedule.split_whitespace().collect::<Vec<_>>();
    let minute = fields[0].parse::<u8>().ok();
    let hour = fields[1].parse::<u8>().ok();
    if minute.is_none() || hour.is_none() || !fields[2..].iter().all(|field| *field == "*") {
        return Err(AppError::Validation(
            "window alignment must be a single local daily time".to_string(),
        ));
    }
    Ok(())
}

fn local_date(epoch_seconds: i64) -> AppResult<Date> {
    let utc = OffsetDateTime::from_unix_timestamp(epoch_seconds)
        .map_err(|error| AppError::Validation(format!("invalid schedule time: {error}")))?;
    let offset = time::UtcOffset::local_offset_at(utc)
        .map_err(|error| AppError::Internal(format!("read local timezone offset: {error}")))?;
    Ok(utc.to_offset(offset).date())
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
    request_logger: &OutboundRequestLogger,
) -> Result<Vec<ProviderTriggerModel>, ProviderTriggerError> {
    let response = request_logger
        .send(
            http_client()
                .get(CLAUDE_CODE_MODELS_URL)
                .bearer_auth(access_token)
                .header("anthropic-version", "2023-06-01")
                .header("anthropic-beta", "oauth-2025-04-20")
                .header("Accept", "application/json"),
            provider_trigger_log_context("claude_code_models", "GET", CLAUDE_CODE_MODELS_URL),
        )
        .await
        .map_err(provider_trigger_request_error)?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    ensure_success_status_or_trigger_error(status, &body, "Claude Code models")?;
    let parsed = serde_json::from_str::<ClaudeModelsResponse>(&body)
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;

    let mut models = parsed
        .data
        .into_iter()
        .map(|model| ProviderTriggerModel {
            display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
        })
        .collect::<Vec<_>>();
    models.sort_by(|a, b| claude_trigger_model_rank(&a.id).cmp(&claude_trigger_model_rank(&b.id)));
    Ok(models)
}

async fn trigger_claude_code(
    access_token: &str,
    model_id: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<ProviderTriggerOutcome, ProviderTriggerError> {
    let response = request_logger
        .send(
            http_client()
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
                        "messages": [{"role": "user", "content": WINDOW_ALIGN_PROMPT}]
                    })
                    .to_string(),
                ),
            provider_trigger_log_context("claude_code_messages", "POST", CLAUDE_CODE_MESSAGES_URL),
        )
        .await
        .map_err(provider_trigger_request_error)?;
    let status = response.status();
    let headers = response.headers().clone();
    let rate_limit_rejection = claude_rate_limit_rejection_message(&headers);
    let body = response
        .text()
        .await
        .map_err(|error| ProviderTriggerError::Retryable(error.to_string()))?;
    if !status.is_success() {
        let _ = request_logger.write_http_response_detail(
            provider_trigger_log_context(
                "claude_code_messages_detail",
                "POST",
                CLAUDE_CODE_MESSAGES_URL,
            ),
            status,
            &headers,
            &body,
            Some(claude_trigger_request_metadata(model_id)),
        );
    }
    if let Some(message) = rate_limit_rejection {
        return Err(ProviderTriggerError::Terminal(message));
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let detail = response_error_detail(&body);
        let message = if is_haiku_model(model_id) {
            if detail.is_empty() {
                format!("Claude Code model {model_id} returned rate limit")
            } else {
                format!("Claude Code model {model_id} returned rate limit: {detail}")
            }
        } else {
            format!(
                "Claude Code model {model_id} returned rate limit; use a Haiku model for window alignment"
            )
        };
        return Err(ProviderTriggerError::Retryable(message));
    }
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

fn claude_trigger_model_rank(model_id: &str) -> (u8, &str) {
    let tier = if is_haiku_model(model_id) {
        0
    } else if model_id.contains("sonnet") {
        1
    } else if model_id.contains("opus") {
        2
    } else {
        3
    };
    (tier, model_id)
}

fn is_haiku_model(model_id: &str) -> bool {
    model_id.contains("haiku")
}

fn claude_trigger_request_metadata(model_id: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("modelId".to_string(), model_id.to_string()),
        ("maxTokens".to_string(), "1".to_string()),
        (
            "promptChars".to_string(),
            WINDOW_ALIGN_PROMPT.chars().count().to_string(),
        ),
    ])
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
        return Err(ProviderTriggerError::AuthRequired);
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return Err(ProviderTriggerError::Retryable(message));
    }
    Err(ProviderTriggerError::Terminal(message))
}

fn claude_rate_limit_rejection_message(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let status = headers
        .get(CLAUDE_UNIFIED_STATUS_HEADER)
        .and_then(|value| value.to_str().ok())?;
    if status != "rejected" {
        return None;
    }

    let claim = headers
        .get(CLAUDE_UNIFIED_CLAIM_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("quota");
    let window = match claim {
        "five_hour" => "5-hour limit",
        "seven_day" => "weekly limit",
        _ => "quota",
    };
    let reset = headers
        .get(CLAUDE_UNIFIED_RESET_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(unix_seconds_to_local_label);

    Some(match reset {
        Some(reset) => format!("Claude Code {window} is exhausted; resets at {reset}"),
        None => format!("Claude Code {window} is exhausted"),
    })
}

fn unix_seconds_to_local_label(epoch_seconds: i64) -> Option<String> {
    let datetime = OffsetDateTime::from_unix_timestamp(epoch_seconds).ok()?;
    datetime.format(&Rfc3339).ok()
}

fn provider_trigger_log_context(
    operation: &'static str,
    method: &'static str,
    url: &str,
) -> OutboundRequestContext {
    OutboundRequestContext {
        category: "provider_trigger",
        operation,
        provider_id: Some(CLAUDE_CODE_PROVIDER_ID.to_string()),
        method,
        url: url.to_string(),
    }
}

fn provider_trigger_request_error(error: OutboundRequestError) -> ProviderTriggerError {
    match error {
        OutboundRequestError::Http(error) => ProviderTriggerError::Retryable(error.to_string()),
        OutboundRequestError::Log(error) => ProviderTriggerError::Terminal(error.to_string()),
    }
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
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
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

    struct BlockingRunner {
        trigger_count: AtomicUsize,
        release: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    }

    impl BlockingRunner {
        fn new(release: tokio::sync::oneshot::Receiver<()>) -> Self {
            Self {
                trigger_count: AtomicUsize::new(0),
                release: Mutex::new(Some(release)),
            }
        }
    }

    impl ProviderTriggerRunner for BlockingRunner {
        fn supports(&self, _provider_id: &str) -> bool {
            true
        }

        fn list_models<'a>(&'a self, _provider_id: &'a str) -> ProviderModelsFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn trigger<'a>(
            &'a self,
            _provider_id: &'a str,
            _model_id: &'a str,
        ) -> ProviderTriggerFuture<'a> {
            Box::pin(async move {
                self.trigger_count.fetch_add(1, Ordering::SeqCst);
                let release = self.release.lock().expect("lock release").take();
                if let Some(release) = release {
                    let _ = release.await;
                }
                Ok(ProviderTriggerOutcome {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                })
            })
        }
    }

    fn test_service(runner: FakeRunner) -> ProviderTriggerService {
        let db = Arc::new(Database::open_in_memory().expect("open db"));
        ProviderTriggerService::with_runner(db, Arc::new(runner))
    }

    fn next_minute_start() -> i64 {
        let now = current_epoch_seconds();
        now - now.rem_euclid(60) + 60
    }

    fn daily_cron_for(epoch_seconds: i64) -> String {
        let utc = OffsetDateTime::from_unix_timestamp(epoch_seconds).expect("test timestamp");
        let offset = time::UtcOffset::local_offset_at(utc).expect("local offset");
        let local = utc.to_offset(offset);
        format!("{} {} * * *", local.minute(), local.hour())
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
                window_align_next_attempt_at: None,
                window_align_last_attempt_at: None,
                window_align_last_status: "never".to_string(),
                window_align_last_error: None,
            }
        );
    }

    #[test]
    fn claude_rate_limit_rejection_maps_to_terminal_quota_message() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            CLAUDE_UNIFIED_STATUS_HEADER,
            reqwest::header::HeaderValue::from_static("rejected"),
        );
        headers.insert(
            CLAUDE_UNIFIED_CLAIM_HEADER,
            reqwest::header::HeaderValue::from_static("five_hour"),
        );
        headers.insert(
            CLAUDE_UNIFIED_RESET_HEADER,
            reqwest::header::HeaderValue::from_static("1787936400"),
        );

        let message = claude_rate_limit_rejection_message(&headers).expect("quota rejection");

        assert!(message.contains("5-hour limit is exhausted"));
        assert!(!message.contains("1787936400"));
    }

    #[test]
    fn claude_trigger_model_rank_prefers_haiku_for_window_alignment() {
        let mut models = [
            "claude-sonnet-4-6",
            "claude-opus-4-8",
            "claude-haiku-4-5-20251001",
        ];

        models.sort_by(|a, b| claude_trigger_model_rank(a).cmp(&claude_trigger_model_rank(b)));

        assert_eq!(models[0], "claude-haiku-4-5-20251001");
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
                window_align_next_attempt_at: None,
                window_align_last_attempt_at: None,
                window_align_last_status: "never".to_string(),
                window_align_last_error: None,
            },
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retryable_failure_schedules_five_minute_retry() {
        let now = next_minute_start();
        let schedule = daily_cron_for(now);
        let service = test_service(FakeRunner::supported_with(vec![Err(
            ProviderTriggerError::Retryable("temporary failure".to_string()),
        )]));
        service
            .set_provider_schedule_settings(
                "claude",
                ProviderScheduleSettings {
                    quota_refresh_minutes: 5,
                    window_align_cron: schedule,
                    window_align_model_id: Some("model-1".to_string()),
                    window_align_next_attempt_at: None,
                    window_align_last_attempt_at: None,
                    window_align_last_status: "never".to_string(),
                    window_align_last_error: None,
                },
            )
            .expect("save schedule");

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
        let now = next_minute_start();
        let schedule = daily_cron_for(now);
        let service = test_service(FakeRunner::supported_with(vec![Err(
            ProviderTriggerError::Terminal("auth expired".to_string()),
        )]));
        service
            .set_provider_schedule_settings(
                "claude",
                ProviderScheduleSettings {
                    quota_refresh_minutes: 5,
                    window_align_cron: schedule,
                    window_align_model_id: Some("model-1".to_string()),
                    window_align_next_attempt_at: None,
                    window_align_last_attempt_at: None,
                    window_align_last_status: "never".to_string(),
                    window_align_last_error: None,
                },
            )
            .expect("save schedule");

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
        let now = next_minute_start();
        let schedule = daily_cron_for(now);
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
                    window_align_cron: schedule.clone(),
                    window_align_model_id: Some("model-1".to_string()),
                    window_align_next_attempt_at: None,
                    window_align_last_attempt_at: None,
                    window_align_last_status: "never".to_string(),
                    window_align_last_error: None,
                },
            )
            .expect("save schedule");

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
            Some(
                next_window_alignment_attempt_after_success(&schedule, now).expect("next attempt")
            )
        );
    }

    #[test]
    fn success_that_rolls_into_tomorrow_uses_later_of_daily_start_and_cooldown() {
        let mut success_at = next_minute_start();
        for offset_minutes in 0..(3 * 24 * 60) {
            let candidate = success_at + offset_minutes * 60;
            let utc = OffsetDateTime::from_unix_timestamp(candidate).expect("test timestamp");
            let offset = time::UtcOffset::local_offset_at(utc).expect("local offset");
            let local = utc.to_offset(offset);
            if local.hour() == 22 && local.minute() == 0 {
                success_at = candidate;
                break;
            }
        }

        let next_attempt = next_window_alignment_attempt_after_success("0 5 * * *", success_at)
            .expect("next attempt");
        let cooldown_at = success_at + PROVIDER_WINDOW_SECONDS;
        let daily_start =
            next_cron_occurrence_after_local("0 5 * * *", success_at).expect("daily start");

        assert_eq!(
            local_date(cooldown_at).expect("cooldown date")
                > local_date(success_at).expect("success date"),
            true
        );
        assert_eq!(next_attempt, daily_start.max(cooldown_at));
    }

    #[tokio::test]
    async fn manual_trigger_records_last_attempt_result() {
        let service = test_service(FakeRunner::supported_with(vec![Ok(
            ProviderTriggerOutcome {
                prompt_tokens: 1,
                completion_tokens: 1,
            },
        )]));

        let settings = service
            .run_window_alignment_now("claude", "model-1")
            .await
            .expect("run manual trigger");

        assert_eq!(settings.window_align_last_status, "success");
        assert!(settings.window_align_last_attempt_at.is_some());
        assert_eq!(settings.window_align_last_error, None);
        assert_eq!(settings.window_align_cron, "");
        assert_eq!(settings.window_align_model_id, None);
    }

    #[tokio::test]
    async fn concurrent_manual_triggers_for_same_provider_are_coalesced() {
        let db = Arc::new(Database::open_in_memory().expect("open db"));
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let runner = Arc::new(BlockingRunner::new(release_rx));
        let service = ProviderTriggerService::with_runner(db, runner.clone());

        let first = {
            let service = service.clone();
            tokio::spawn(async move { service.run_window_alignment_now("claude", "model-1").await })
        };

        while runner.trigger_count.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        let second = service
            .run_window_alignment_now("claude", "model-1")
            .await
            .expect("coalesced manual trigger reads current settings");
        assert_eq!(second.window_align_last_status, "never");
        assert_eq!(runner.trigger_count.load(Ordering::SeqCst), 1);

        release_tx.send(()).expect("release first trigger");
        let first = first
            .await
            .expect("first task joins")
            .expect("first trigger succeeds");
        assert_eq!(first.window_align_last_status, "success");
        assert_eq!(runner.trigger_count.load(Ordering::SeqCst), 1);
    }
}
