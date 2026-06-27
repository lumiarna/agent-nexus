use serde::Deserialize;

use crate::{
    error::AppResult,
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

use super::{
    super::{
        shared::{
            http_client, percent_to_u8, provider_quota_log_context, provider_quota_request_error,
            shortest_percent_window_used,
        },
        ProviderCredentialSource, ProviderQuotaAdapter, ProviderQuotaFuture,
        ProviderQuotaPollError, ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow,
        ProviderQuotaWindowKind, ProviderUsageTransport,
    },
    opencode_custom,
};

pub(crate) const PROVIDER_ID: &str = "copilot";
const USAGE_URL: &str = "https://api.github.com/copilot_internal/user";

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

pub(crate) struct CopilotQuotaAdapter;

impl ProviderQuotaAdapter for CopilotQuotaAdapter {
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
            let token = match credential_source.copilot_token(app_config) {
                Ok(Some(token)) => token,
                Ok(None) => return status(ProviderQuotaStatus::NoCreds, "not found"),
                Err(error) => {
                    return status(ProviderQuotaStatus::Failed, error.to_string().as_str())
                }
            };

            let usage = usage_transport.copilot_usage(&token).await;
            derive_snapshot(usage)
        })
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

    let primary = shortest_percent_window_used(&windows);
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

fn derive_snapshot(
    usage: Result<CopilotUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match usage {
        Ok(response) => {
            let mut snapshot = copilot_quota_from_usage_response(PROVIDER_ID, response);
            snapshot.credential = Some("GitHub Copilot token".to_string());
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
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
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan: None,
            primary: None,
            windows: Vec::new(),
            credential: Some("GitHub Copilot token".to_string()),
            error: Some(error.to_string()),
        },
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
        value_label: None,
        value_only: false,
        reset_at,
        unlimited: detail.unlimited == Some(true),
    })
}

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

pub(crate) fn read_token(app_config: &AppConfigService) -> AppResult<Option<String>> {
    if let Some(token) = app_config
        .get_copilot_github_token()?
        .filter(|token| !token.is_empty())
    {
        return Ok(Some(token));
    }
    Ok(opencode_custom::read_opencode_copilot_token())
}

pub(crate) async fn fetch_usage(
    token: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<CopilotUsageResponse, ProviderQuotaPollError> {
    let response = request_logger
        .send(
            http_client()
                .get(USAGE_URL)
                .header("Authorization", format!("token {token}"))
                .header("Accept", "application/json")
                .header("Editor-Version", "vscode/1.96.2")
                .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
                .header("User-Agent", "GitHubCopilotChat/0.26.7")
                .header("X-Github-Api-Version", "2025-04-01"),
            provider_quota_log_context("copilot_usage", PROVIDER_ID, "GET", USAGE_URL),
        )
        .await
        .map_err(provider_quota_request_error)?;

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
