use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    error::{AppError, AppResult},
    services::app_config::AppConfigService,
};

const CLAUDE_CODE_PROVIDER_ID: &str = "claude";
const CLAUDE_CODE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

const CODEX_PROVIDER_ID: &str = "codex";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

const COPILOT_PROVIDER_ID: &str = "copilot";
const COPILOT_USAGE_URL: &str = "https://api.github.com/copilot_internal/user";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderQuotaStatus {
    Available,
    Expired,
    Failed,
    #[serde(rename = "nocreds")]
    NoCreds,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderQuotaWindowKind {
    Rolling,
    Weekly,
    Monthly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuotaWindow {
    pub label: String,
    pub kind: ProviderQuotaWindowKind,
    pub used: u8,
    pub reset_at: Option<String>,
    pub unlimited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuotaSnapshot {
    pub provider_id: String,
    pub status: ProviderQuotaStatus,
    pub plan: Option<String>,
    pub primary: Option<u8>,
    pub windows: Vec<ProviderQuotaWindow>,
    pub credential: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClaudeCodeUsageResponse {
    pub five_hour: Option<ClaudeCodeUsageBucket>,
    pub seven_day: Option<ClaudeCodeUsageBucket>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClaudeCodeUsageBucket {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexUsageResponse {
    pub plan_type: Option<String>,
    pub rate_limit: Option<CodexRateLimit>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexRateLimit {
    pub primary_window: Option<CodexRateLimitWindow>,
    pub secondary_window: Option<CodexRateLimitWindow>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexRateLimitWindow {
    pub used_percent: Option<f64>,
    pub limit_window_seconds: Option<i64>,
    pub reset_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotUsageResponse {
    pub copilot_plan: Option<String>,
    pub quota_reset_date: Option<String>,
    pub quota_snapshots: Option<CopilotQuotaSnapshots>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotQuotaSnapshots {
    pub premium_interactions: Option<CopilotQuotaDetail>,
    pub chat: Option<CopilotQuotaDetail>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotQuotaDetail {
    pub entitlement: Option<i64>,
    pub remaining: Option<f64>,
    pub percent_remaining: Option<f64>,
    pub unlimited: Option<bool>,
}

#[derive(Clone, Debug)]
struct ClaudeCodeCredentials {
    access_token: String,
    expires_at: Option<i64>,
    plan: Option<String>,
    source: String,
}

#[derive(Clone)]
pub struct ProviderQuotaService {
    app_config: AppConfigService,
}

impl ProviderQuotaService {
    pub fn new(app_config: AppConfigService) -> Self {
        Self { app_config }
    }

    pub async fn get_provider_quota(&self, provider_id: &str) -> AppResult<ProviderQuotaSnapshot> {
        match provider_id {
            CLAUDE_CODE_PROVIDER_ID | "claude-code" => Ok(self.get_claude_code_quota().await),
            CODEX_PROVIDER_ID => Ok(self.get_codex_quota().await),
            COPILOT_PROVIDER_ID => Ok(self.get_copilot_quota().await),
            _ => Err(AppError::Validation("unsupported provider".to_string())),
        }
    }

    pub async fn get_claude_code_quota(&self) -> ProviderQuotaSnapshot {
        let credentials = match self.read_claude_code_credentials() {
            Ok(Some(credentials)) => credentials,
            Ok(None) => return claude_code_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return claude_code_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        if is_token_expired(credentials.expires_at) {
            return ProviderQuotaSnapshot {
                provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Expired,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some("Claude Code token expired; run claude /login to refresh".to_string()),
            };
        }

        match fetch_claude_code_usage(&credentials.access_token).await {
            Ok(response) => {
                let mut snapshot = claude_code_quota_from_usage_response(
                    CLAUDE_CODE_PROVIDER_ID,
                    credentials.plan,
                    response,
                );
                snapshot.credential = Some(credentials.source);
                snapshot
            }
            Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
                provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Expired,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some(
                    "Claude Code authorization was rejected; run claude /login".to_string(),
                ),
            },
            Err(error) => ProviderQuotaSnapshot {
                provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Failed,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some(error.to_string()),
            },
        }
    }

    pub async fn get_codex_quota(&self) -> ProviderQuotaSnapshot {
        let credentials = match self.read_codex_credentials() {
            Ok(Some(credentials)) => credentials,
            Ok(None) => return codex_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return codex_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        match fetch_codex_usage(&credentials.access_token, credentials.account_id.as_deref()).await
        {
            Ok(response) => {
                let mut snapshot =
                    codex_quota_from_usage_response(CODEX_PROVIDER_ID, credentials.plan, response);
                snapshot.credential = Some(credentials.source);
                snapshot
            }
            Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
                provider_id: CODEX_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Expired,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some(
                    "Codex authorization was rejected; run codex login to refresh".to_string(),
                ),
            },
            Err(error) => ProviderQuotaSnapshot {
                provider_id: CODEX_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Failed,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some(error.to_string()),
            },
        }
    }

    pub async fn get_copilot_quota(&self) -> ProviderQuotaSnapshot {
        let token = match self.read_copilot_github_token() {
            Ok(Some(token)) => token,
            Ok(None) => return copilot_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return copilot_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        match fetch_copilot_usage(&token).await {
            Ok(response) => {
                let mut snapshot = copilot_quota_from_usage_response(COPILOT_PROVIDER_ID, response);
                snapshot.credential = Some("GitHub Copilot token".to_string());
                snapshot
            }
            Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
                provider_id: COPILOT_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Expired,
                plan: None,
                primary: None,
                windows: Vec::new(),
                credential: Some("GitHub Copilot token".to_string()),
                error: Some(
                    "GitHub Copilot token was rejected; provide a valid Copilot-scoped token"
                        .to_string(),
                ),
            },
            Err(error) => ProviderQuotaSnapshot {
                provider_id: COPILOT_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Failed,
                plan: None,
                primary: None,
                windows: Vec::new(),
                credential: Some("GitHub Copilot token".to_string()),
                error: Some(error.to_string()),
            },
        }
    }

    fn read_copilot_github_token(&self) -> AppResult<Option<String>> {
        if let Some(token) = self
            .app_config
            .get_copilot_github_token()?
            .filter(|token| !token.is_empty())
        {
            return Ok(Some(token));
        }
        Ok(read_opencode_copilot_token())
    }

    fn read_codex_credentials(&self) -> AppResult<Option<CodexCredentials>> {
        let path = self.app_config.get_codex_config_dir()?.join("auth.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        parse_codex_credentials(&content, &path)
    }

    fn read_claude_code_credentials(&self) -> AppResult<Option<ClaudeCodeCredentials>> {
        #[cfg(target_os = "macos")]
        if let Some(credentials) = read_claude_code_credentials_from_keychain()? {
            return Ok(Some(credentials));
        }

        let path = self
            .app_config
            .get_claude_config_dir()?
            .join(".credentials.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        parse_claude_code_credentials(&content, path)
    }
}

pub fn claude_code_quota_from_usage_response(
    provider_id: &str,
    plan: Option<String>,
    response: ClaudeCodeUsageResponse,
) -> ProviderQuotaSnapshot {
    let windows = [
        response
            .five_hour
            .map(|bucket| quota_window("5-hour limit", ProviderQuotaWindowKind::Rolling, bucket)),
        response
            .seven_day
            .map(|bucket| quota_window("Weekly limit", ProviderQuotaWindowKind::Weekly, bucket)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let primary = windows.iter().map(|window| window.used).max();

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

pub fn codex_quota_from_usage_response(
    provider_id: &str,
    plan: Option<String>,
    response: CodexUsageResponse,
) -> ProviderQuotaSnapshot {
    let mut windows = Vec::new();
    if let Some(rate_limit) = response.rate_limit {
        for window in [rate_limit.primary_window, rate_limit.secondary_window]
            .into_iter()
            .flatten()
        {
            if let Some(used_percent) = window.used_percent {
                let (kind, label) = codex_window_meta(window.limit_window_seconds);
                windows.push(ProviderQuotaWindow {
                    label,
                    kind,
                    used: percent_to_u8(used_percent),
                    reset_at: window.reset_at.and_then(unix_seconds_to_iso),
                    unlimited: false,
                });
            }
        }
    }

    let primary = windows.iter().map(|window| window.used).max();

    let plan = response
        .plan_type
        .map(|raw| codex_plan_label(&raw))
        .or(plan);

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

pub fn copilot_quota_from_usage_response(
    provider_id: &str,
    response: CopilotUsageResponse,
) -> ProviderQuotaSnapshot {
    let reset_at = response
        .quota_reset_date
        .as_deref()
        .and_then(copilot_reset_to_iso);
    let snapshots = response.quota_snapshots;

    let windows = [
        snapshots
            .as_ref()
            .and_then(|s| s.premium_interactions.as_ref())
            .and_then(|detail| copilot_window("Premium Interactions", detail, reset_at.clone())),
        snapshots
            .as_ref()
            .and_then(|s| s.chat.as_ref())
            .and_then(|detail| copilot_window("Chat Quota", detail, reset_at.clone())),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let primary = windows.iter().map(|window| window.used).max();
    let plan = response.copilot_plan.as_deref().map(copilot_plan_label);

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

fn copilot_window(
    label: &str,
    detail: &CopilotQuotaDetail,
    reset_at: Option<String>,
) -> Option<ProviderQuotaWindow> {
    let remaining = copilot_remaining_percent(detail)?;
    Some(ProviderQuotaWindow {
        label: label.to_string(),
        kind: ProviderQuotaWindowKind::Monthly,
        used: percent_to_u8(100.0 - remaining),
        reset_at,
        unlimited: detail.unlimited == Some(true),
    })
}

/// Copilot reports remaining quota; an unlimited window has no usable percentage
/// so it is treated as fully available (0% used).
fn copilot_remaining_percent(detail: &CopilotQuotaDetail) -> Option<f64> {
    if detail.unlimited == Some(true) {
        return Some(100.0);
    }
    if let Some(percent) = detail.percent_remaining {
        return Some(percent.clamp(0.0, 100.0));
    }
    let entitlement = detail.entitlement? as f64;
    let remaining = detail.remaining?;
    if entitlement > 0.0 {
        Some((remaining / entitlement * 100.0).clamp(0.0, 100.0))
    } else {
        None
    }
}

fn copilot_plan_label(plan: &str) -> String {
    let normalized = plan.to_lowercase();
    if normalized.contains("business") {
        "Copilot Business".to_string()
    } else if normalized.contains("enterprise") {
        "Copilot Enterprise".to_string()
    } else if normalized.contains("free") {
        "Copilot Free".to_string()
    } else if normalized.contains("pro") || normalized.contains("individual") {
        "Copilot Pro".to_string()
    } else if normalized.is_empty() {
        "Copilot".to_string()
    } else {
        format!("Copilot {plan}")
    }
}

/// `quota_reset_date` is a calendar date ("2026-07-01"); anchor a bare date to
/// midnight UTC so the frontend can render the exact month/day.
fn copilot_reset_to_iso(date: &str) -> Option<String> {
    let date = date.trim();
    if date.is_empty() {
        return None;
    }
    if date.contains('T') {
        return Some(date.to_string());
    }
    if is_iso_calendar_date(date) {
        Some(format!("{date}T00:00:00Z"))
    } else {
        None
    }
}

fn is_iso_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && value[0..4].bytes().all(|c| c.is_ascii_digit())
        && value[5..7].bytes().all(|c| c.is_ascii_digit())
        && value[8..10].bytes().all(|c| c.is_ascii_digit())
}

/// Resolve the GitHub token from opencode's `auth.json`. The location honours
/// `OPENCODE_AUTH_FILE`, defaulting to `~/.local/share/opencode/auth.json`.
fn read_opencode_copilot_token() -> Option<String> {
    let path = match env::var_os("OPENCODE_AUTH_FILE") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => crate::services::paths::home_dir()?
            .join(".local")
            .join("share")
            .join("opencode")
            .join("auth.json"),
    };
    let content = fs::read_to_string(path).ok()?;
    parse_opencode_copilot_token(&content)
}

/// Extract the `github-copilot` GitHub token from opencode `auth.json` content.
/// OAuth providers store it under `access`; API providers under `key`.
pub fn parse_opencode_copilot_token(content: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let entry = json.get("github-copilot")?;
    for field in ["access", "key"] {
        if let Some(token) = entry.get(field).and_then(|value| value.as_str()) {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn codex_window_meta(limit_window_seconds: Option<i64>) -> (ProviderQuotaWindowKind, String) {
    match limit_window_seconds {
        Some(18000) => (ProviderQuotaWindowKind::Rolling, "5-hour limit".to_string()),
        Some(604800) => (ProviderQuotaWindowKind::Weekly, "Weekly limit".to_string()),
        Some(secs) => {
            let hours = secs / 3600;
            let kind = if secs >= 604800 {
                ProviderQuotaWindowKind::Weekly
            } else {
                ProviderQuotaWindowKind::Rolling
            };
            let label = if hours >= 24 {
                format!("{}-day limit", hours / 24)
            } else {
                format!("{}-hour limit", hours)
            };
            (kind, label)
        }
        None => (ProviderQuotaWindowKind::Rolling, "Limit".to_string()),
    }
}

fn unix_seconds_to_iso(secs: i64) -> Option<String> {
    OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
}

fn quota_window(
    label: &str,
    kind: ProviderQuotaWindowKind,
    bucket: ClaudeCodeUsageBucket,
) -> ProviderQuotaWindow {
    ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(bucket.utilization),
        reset_at: bucket.resets_at,
        unlimited: false,
    }
}

fn percent_to_u8(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, 100.0) as u8
}

async fn fetch_claude_code_usage(
    access_token: &str,
) -> Result<ClaudeCodeUsageResponse, ProviderQuotaPollError> {
    let response = reqwest::Client::new()
        .get(CLAUDE_CODE_USAGE_URL)
        .bearer_auth(access_token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Claude Code usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<ClaudeCodeUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

#[cfg(target_os = "macos")]
fn read_claude_code_credentials_from_keychain() -> AppResult<Option<ClaudeCodeCredentials>> {
    let output = match Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let content = String::from_utf8(output.stdout)
        .map_err(|error| AppError::Validation(format!("invalid Keychain credentials: {error}")))?;
    parse_claude_code_credentials_from_source(
        content.trim(),
        "macOS Keychain · Claude Code-credentials".to_string(),
    )
}

fn parse_claude_code_credentials(
    content: &str,
    path: PathBuf,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    parse_claude_code_credentials_from_source(content, path_to_display(&path))
}

fn parse_claude_code_credentials_from_source(
    content: &str,
    source: String,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    let json: serde_json::Value = serde_json::from_str(content).map_err(|error| {
        AppError::Validation(format!("invalid Claude Code credentials: {error}"))
    })?;
    let Some(oauth) = json
        .get("claudeAiOauth")
        .or_else(|| json.get("claude.ai_oauth"))
    else {
        return Ok(None);
    };
    let Some(access_token) = oauth.get("accessToken").and_then(|value| value.as_str()) else {
        return Ok(None);
    };
    let expires_at = oauth.get("expiresAt").and_then(|value| value.as_i64());
    let rate_limit_tier = oauth
        .get("rateLimitTier")
        .or_else(|| oauth.get("rate_limit_tier"))
        .and_then(|value| value.as_str());
    let subscription_type = oauth
        .get("subscriptionType")
        .or_else(|| oauth.get("subscription_type"))
        .and_then(|value| value.as_str());

    Ok(Some(ClaudeCodeCredentials {
        access_token: access_token.to_string(),
        expires_at,
        plan: infer_claude_code_plan(rate_limit_tier, subscription_type),
        source,
    }))
}

fn infer_claude_code_plan(
    rate_limit_tier: Option<&str>,
    subscription_type: Option<&str>,
) -> Option<String> {
    let tier = rate_limit_tier.unwrap_or_default().to_lowercase();
    let subscription = subscription_type.unwrap_or_default().to_lowercase();

    for hint in [&subscription, &tier] {
        if hint.contains("max") {
            return Some("Claude Max".to_string());
        }
        if hint.contains("pro") {
            return Some("Claude Pro".to_string());
        }
        if hint.contains("team") {
            return Some("Claude Team".to_string());
        }
        if hint.contains("enterprise") {
            return Some("Claude Enterprise".to_string());
        }
    }

    if subscription.is_empty() && tier.is_empty() {
        None
    } else {
        Some("Claude".to_string())
    }
}

fn is_token_expired(expires_at: Option<i64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    now >= expires_at
}

fn path_to_display(path: &Path) -> String {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return path.to_string_lossy().into_owned();
    };
    match path.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.to_string_lossy()),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn claude_code_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

#[derive(Clone, Debug)]
struct CodexCredentials {
    access_token: String,
    account_id: Option<String>,
    plan: Option<String>,
    source: String,
}

fn parse_codex_credentials(content: &str, path: &Path) -> AppResult<Option<CodexCredentials>> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|error| AppError::Validation(format!("invalid Codex auth.json: {error}")))?;

    let tokens = json.get("tokens");
    let access_token = tokens
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(|value| value.as_str());
    let Some(access_token) = access_token else {
        return Ok(None);
    };
    let account_id = tokens
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let id_token = tokens
        .and_then(|tokens| tokens.get("id_token"))
        .and_then(|value| value.as_str());

    let plan = id_token.and_then(decode_codex_plan_from_id_token);

    Ok(Some(CodexCredentials {
        access_token: access_token.to_string(),
        account_id,
        plan,
        source: path_to_display(path),
    }))
}

fn decode_codex_plan_from_id_token(id_token: &str) -> Option<String> {
    let payload = decode_jwt_payload(id_token)?;
    let plan_type = payload
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(|value| value.as_str())?;
    Some(codex_plan_label(plan_type))
}

fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let mut parts = token.split('.');
    parts.next()?;
    let payload_b64 = parts.next()?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload_b64))
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn codex_plan_label(plan_type: &str) -> String {
    let plan = plan_type.to_lowercase();
    if plan.contains("pro") {
        "ChatGPT Pro".to_string()
    } else if plan.contains("plus") {
        "ChatGPT Plus".to_string()
    } else if plan.contains("team") {
        "ChatGPT Team".to_string()
    } else if plan.contains("enterprise") {
        "ChatGPT Enterprise".to_string()
    } else if plan.contains("business") {
        "ChatGPT Business".to_string()
    } else if plan.is_empty() {
        "ChatGPT".to_string()
    } else {
        format!("ChatGPT {}", plan_type)
    }
}

async fn fetch_codex_usage(
    access_token: &str,
    account_id: Option<&str>,
) -> Result<CodexUsageResponse, ProviderQuotaPollError> {
    let mut request = reqwest::Client::new()
        .get(CODEX_USAGE_URL)
        .bearer_auth(access_token)
        .header("User-Agent", "codex-cli")
        .header("Accept", "application/json");
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Codex usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<CodexUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn codex_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: CODEX_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

async fn fetch_copilot_usage(token: &str) -> Result<CopilotUsageResponse, ProviderQuotaPollError> {
    let response = reqwest::Client::new()
        .get(COPILOT_USAGE_URL)
        .header("Authorization", format!("token {token}"))
        .header("Accept", "application/json")
        .header("Editor-Version", "vscode/1.96.2")
        .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
        .header("User-Agent", "GitHubCopilotChat/0.26.7")
        .header("X-Github-Api-Version", "2025-04-01")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Copilot usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<CopilotUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn copilot_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: COPILOT_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

#[derive(Debug, thiserror::Error)]
enum ProviderQuotaPollError {
    #[error("Claude Code authorization failed")]
    AuthRequired,
    #[error("{0}")]
    Request(String),
}
