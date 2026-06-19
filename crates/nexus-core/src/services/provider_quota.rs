use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    services::app_config::AppConfigService,
};

const CLAUDE_CODE_PROVIDER_ID: &str = "claude";
const CLAUDE_CODE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuotaWindow {
    pub label: String,
    pub kind: ProviderQuotaWindowKind,
    pub used: u8,
    pub reset_at: Option<String>,
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

#[derive(Debug, thiserror::Error)]
enum ProviderQuotaPollError {
    #[error("Claude Code authorization failed")]
    AuthRequired,
    #[error("{0}")]
    Request(String),
}
