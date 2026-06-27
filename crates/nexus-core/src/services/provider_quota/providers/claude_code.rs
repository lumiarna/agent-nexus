use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use std::process::Command;

use serde::Deserialize;

use crate::{
    error::{AppError, AppResult},
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

use super::super::{
    shared::{
        http_client, percent_to_u8, provider_quota_log_context, provider_quota_request_error,
        shortest_percent_window_used,
    },
    ClaudeAccessToken, ClaudeAuthError, ProviderCredentialSource, ProviderQuotaAdapter,
    ProviderQuotaFuture, ProviderQuotaPollError, ProviderQuotaSnapshot, ProviderQuotaStatus,
    ProviderQuotaWindow, ProviderQuotaWindowKind, ProviderUsageTransport,
};

pub(crate) const PROVIDER_ID: &str = "claude";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const OAUTH_REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";

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
pub(crate) struct ClaudeCodeCredentials {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_at: Option<i64>,
    pub(crate) scopes: Vec<String>,
    pub(crate) plan: Option<String>,
    pub(crate) source: String,
    pub(crate) credentials_path: Option<PathBuf>,
    pub(crate) keychain_account: Option<String>,
    pub(crate) raw: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ClaudeOAuthRefreshResponse {
    pub(crate) access_token: String,
    #[serde(default)]
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_in: i64,
}

pub(crate) struct ClaudeCodeQuotaAdapter;

impl ProviderQuotaAdapter for ClaudeCodeQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["claude-code"]
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_snapshot(app_config, credential_source, usage_transport)
                .await
        })
    }
}

impl ClaudeCodeQuotaAdapter {
    pub(crate) async fn quota_snapshot(
        &self,
        app_config: &AppConfigService,
        credential_source: &dyn ProviderCredentialSource,
        usage_transport: &dyn ProviderUsageTransport,
    ) -> ProviderQuotaSnapshot {
        let auth = ClaudeAccessToken::new(app_config, credential_source, usage_transport);
        let (credentials, access_token) = match auth.acquire().await {
            Ok(result) => result,
            Err(error) => return snapshot_from_auth_error(error),
        };

        let usage = auth
            .with_auth_retry(
                &credentials,
                access_token,
                |access_token| async move {
                    usage_transport.claude_code_usage(&access_token).await
                },
                |error| matches!(error, ProviderQuotaPollError::AuthRequired),
            )
            .await;

        derive_snapshot(credentials, usage)
    }
}

fn snapshot_from_auth_error(error: ClaudeAuthError) -> ProviderQuotaSnapshot {
    let message = error.message();
    match error {
        ClaudeAuthError::NoCreds => status(ProviderQuotaStatus::NoCreds, "not found"),
        ClaudeAuthError::Terminal(_) => status(ProviderQuotaStatus::Failed, &message),
        ClaudeAuthError::MissingScope { credentials } => {
            let ClaudeCodeCredentials { plan, source, .. } = credentials;
            ProviderQuotaSnapshot {
                provider_id: PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Failed,
                plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(source),
                error: Some(message),
            }
        }
        ClaudeAuthError::RefreshFailed { credentials, error } => {
            derive_snapshot(credentials, Err(error))
        }
        ClaudeAuthError::RefreshRejected { credentials } => {
            derive_snapshot(credentials, Err(ProviderQuotaPollError::AuthRequired))
        }
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

    let primary = shortest_percent_window_used(&windows);

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
        value_label: None,
        value_only: false,
        reset_at: bucket.resets_at,
        unlimited: false,
    }
}

fn derive_snapshot(
    credentials: ClaudeCodeCredentials,
    usage: Result<ClaudeCodeUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    let ClaudeCodeCredentials { plan, source, .. } = credentials;
    match usage {
        Ok(response) => {
            let mut snapshot = claude_code_quota_from_usage_response(PROVIDER_ID, plan, response);
            snapshot.credential = Some(source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some("Claude Code authorization was rejected; run claude /login".to_string()),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some(error.to_string()),
        },
    }
}

pub(crate) async fn fetch_usage(
    access_token: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<ClaudeCodeUsageResponse, ProviderQuotaPollError> {
    let response = request_logger
        .send(
            http_client()
                .get(USAGE_URL)
                .bearer_auth(access_token)
                .header("anthropic-beta", "oauth-2025-04-20"),
            provider_quota_log_context("claude_code_usage", PROVIDER_ID, "GET", USAGE_URL),
        )
        .await
        .map_err(provider_quota_request_error)?;

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

pub(crate) async fn fetch_oauth_refresh(
    refresh_token: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", OAUTH_CLIENT_ID),
    ];
    let response = request_logger
        .send(
            http_client()
                .post(OAUTH_REFRESH_URL)
                .header("Accept", "application/json")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .form(&params),
            provider_quota_log_context(
                "claude_oauth_refresh",
                PROVIDER_ID,
                "POST",
                OAUTH_REFRESH_URL,
            ),
        )
        .await
        .map_err(provider_quota_request_error)?;

    if !response.status().is_success() {
        return Err(ProviderQuotaPollError::AuthRequired);
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<ClaudeOAuthRefreshResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

pub(crate) async fn refresh_and_persist_result(
    credentials: &ClaudeCodeCredentials,
    usage_transport: &dyn ProviderUsageTransport,
) -> Result<Option<ClaudeOAuthRefreshResponse>, ProviderQuotaPollError> {
    let Some(refresh_token) = credentials.refresh_token.as_deref() else {
        return Ok(None);
    };
    let refreshed = usage_transport.claude_code_refresh(refresh_token).await?;
    persist_refreshed_credentials(credentials, &refreshed);
    Ok(Some(refreshed))
}

fn persist_refreshed_credentials(
    credentials: &ClaudeCodeCredentials,
    refreshed: &ClaudeOAuthRefreshResponse,
) {
    let mut raw = credentials.raw.clone();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let new_expires_at = now_ms + refreshed.expires_in.saturating_mul(1000);

    let oauth = if let Some(oauth) = raw.get_mut("claudeAiOauth") {
        oauth
    } else if let Some(oauth) = raw.get_mut("claude.ai_oauth") {
        oauth
    } else {
        return;
    };
    oauth["accessToken"] = serde_json::json!(refreshed.access_token);
    if let Some(refresh_token) = &refreshed.refresh_token {
        oauth["refreshToken"] = serde_json::json!(refresh_token);
    }
    oauth["expiresAt"] = serde_json::json!(new_expires_at);

    if let Some(account) = &credentials.keychain_account {
        #[cfg(target_os = "macos")]
        {
            write_keychain_password(KEYCHAIN_SERVICE, account, &raw.to_string());
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = account;
        }
    } else if let Some(path) = &credentials.credentials_path {
        let pretty = serde_json::to_string_pretty(&raw).unwrap_or_else(|_| raw.to_string());
        let _ = fs::write(path, format!("{pretty}\n"));
    }
}

pub(crate) fn read_credentials(
    app_config: &AppConfigService,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    #[cfg(target_os = "macos")]
    if let Some(credentials) = read_claude_code_credentials_from_keychain()? {
        return Ok(Some(credentials));
    }

    let path = app_config
        .get_claude_config_dir()?
        .join(".credentials.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    parse_claude_code_credentials(&content, path)
}

#[cfg(target_os = "macos")]
fn read_keychain_account(service: &str) -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", service, "-g"])
        .stderr(std::process::Stdio::piped())
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    extract_keychain_account(&stderr)
}

fn extract_keychain_account(text: &str) -> Option<String> {
    let needle = "\"acct\"<blob>=\"";
    let start = text.find(needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    let account = &rest[..end];
    if account.is_empty() {
        None
    } else {
        Some(account.to_string())
    }
}

#[cfg(target_os = "macos")]
fn write_keychain_password(service: &str, account: &str, value: &str) {
    let _ = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            account,
            "-s",
            service,
            "-w",
            value,
        ])
        .output();
}

#[cfg(target_os = "macos")]
fn read_claude_code_credentials_from_keychain() -> AppResult<Option<ClaudeCodeCredentials>> {
    let output = match Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
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
    let account = read_keychain_account(KEYCHAIN_SERVICE);
    parse_claude_code_credentials_from_source(
        content.trim(),
        format!("macOS Keychain · {KEYCHAIN_SERVICE}"),
        None,
        account,
    )
}

fn parse_claude_code_credentials(
    content: &str,
    path: PathBuf,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    parse_claude_code_credentials_from_source(content, path_to_display(&path), Some(path), None)
}

fn parse_claude_code_credentials_from_source(
    content: &str,
    source: String,
    credentials_path: Option<PathBuf>,
    keychain_account: Option<String>,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    let Some(json) = try_parse_credential_json(content) else {
        return Ok(None);
    };
    let Some(oauth) = json
        .get("claudeAiOauth")
        .or_else(|| json.get("claude.ai_oauth"))
    else {
        return Ok(None);
    };
    let Some(access_token) = oauth.get("accessToken").and_then(|value| value.as_str()) else {
        return Ok(None);
    };
    let refresh_token = oauth
        .get("refreshToken")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let expires_at = oauth.get("expiresAt").and_then(|value| value.as_i64());
    let scopes = oauth
        .get("scopes")
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
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
        refresh_token,
        expires_at,
        scopes,
        plan: infer_plan(rate_limit_tier, subscription_type),
        source,
        credentials_path,
        keychain_account,
        raw: json,
    }))
}

fn try_parse_credential_json(text: &str) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(value);
    }
    try_decode_hex_json(text)
}

fn try_decode_hex_json(text: &str) -> Option<serde_json::Value> {
    let hex = text.trim();
    let hex = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    serde_json::from_slice(&bytes).ok()
}

fn infer_plan(rate_limit_tier: Option<&str>, subscription_type: Option<&str>) -> Option<String> {
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

pub(crate) fn is_token_expiring_soon(expires_at: Option<i64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    now >= expires_at - 60_000
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

fn status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}
