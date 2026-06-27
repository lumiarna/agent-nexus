use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    error::{AppError, AppResult},
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

use super::super::{
    shared::{
        http_client, percent_to_u8, provider_quota_log_context, provider_quota_request_error,
        shortest_percent_window_used, unix_seconds_to_iso,
    },
    ProviderCredentialSource, ProviderQuotaAdapter, ProviderQuotaFuture, ProviderQuotaPollError,
    ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow, ProviderQuotaWindowKind,
    ProviderUsageTransport,
};

pub(crate) const PROVIDER_ID: &str = "codex";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

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

#[derive(Clone, Debug)]
pub(crate) struct CodexCredentials {
    pub(crate) access_token: String,
    pub(crate) account_id: Option<String>,
    plan: Option<String>,
    source: String,
}

pub(crate) struct CodexQuotaAdapter;

impl ProviderQuotaAdapter for CodexQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            let credentials = match credential_source.codex_credentials(app_config) {
                Ok(Some(credentials)) => credentials,
                Ok(None) => return status(ProviderQuotaStatus::NoCreds, "not found"),
                Err(error) => {
                    return status(ProviderQuotaStatus::Failed, error.to_string().as_str())
                }
            };

            let usage = usage_transport
                .codex_usage(&credentials.access_token, credentials.account_id.as_deref())
                .await;
            derive_snapshot(credentials, usage)
        })
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
                    value_label: None,
                    value_only: false,
                    reset_at: window.reset_at.and_then(unix_seconds_to_iso),
                    unlimited: false,
                });
            }
        }
    }

    let primary = shortest_percent_window_used(&windows);
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

fn derive_snapshot(
    credentials: CodexCredentials,
    usage: Result<CodexUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    let CodexCredentials { plan, source, .. } = credentials;
    match usage {
        Ok(response) => {
            let mut snapshot = codex_quota_from_usage_response(PROVIDER_ID, plan, response);
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
            error: Some("Codex authorization was rejected; run codex login to refresh".to_string()),
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

pub(crate) fn read_credentials(
    app_config: &AppConfigService,
) -> AppResult<Option<CodexCredentials>> {
    let path = app_config.get_codex_config_dir()?.join("auth.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    parse_codex_credentials(&content, &path)
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

    let plan = id_token.and_then(decode_plan_from_id_token);

    Ok(Some(CodexCredentials {
        access_token: access_token.to_string(),
        account_id,
        plan,
        source: path_to_display(path),
    }))
}

fn decode_plan_from_id_token(id_token: &str) -> Option<String> {
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

pub(crate) async fn fetch_usage(
    access_token: &str,
    account_id: Option<&str>,
    request_logger: &OutboundRequestLogger,
) -> Result<CodexUsageResponse, ProviderQuotaPollError> {
    let mut request = http_client()
        .get(USAGE_URL)
        .bearer_auth(access_token)
        .header("User-Agent", "codex-cli")
        .header("Accept", "application/json");
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request_logger
        .send(
            request,
            provider_quota_log_context("codex_usage", PROVIDER_ID, "GET", USAGE_URL),
        )
        .await
        .map_err(provider_quota_request_error)?;

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
