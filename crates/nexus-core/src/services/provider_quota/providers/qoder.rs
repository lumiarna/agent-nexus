use serde::{Deserialize, Serialize};

use crate::{
    error::AppResult,
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

use super::super::{
    shared::{
        http_client, percent_to_u8, provider_quota_log_context, provider_quota_request_error,
        unix_millis_to_iso,
    },
    ProviderCredentialSource, ProviderQuotaAdapter, ProviderQuotaFuture, ProviderQuotaPollError,
    ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow, ProviderQuotaWindowKind,
    ProviderUsageTransport,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QoderUsageResponse {
    pub quota_key: Option<String>,
    pub status: Option<String>,
    pub plan_quota: Option<QoderQuotaBlock>,
    pub resource_package_quota: Option<QoderQuotaBlock>,
    pub total_quota: Option<QoderQuotaBlock>,
    #[serde(alias = "lastResetAt")]
    pub last_reset_at: Option<i64>,
    #[serde(alias = "nextResetAt")]
    pub next_reset_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QoderQuotaBlock {
    pub quota_summary: Option<QoderQuotaSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QoderQuotaSummary {
    pub used_value: Option<f64>,
    pub limit_value: Option<f64>,
    pub remaining_value: Option<f64>,
    pub usage_percentage: Option<f64>,
    pub unit: Option<String>,
}

pub(crate) const PROVIDER_ID: &str = "qoder";
const QUOTA_URL: &str = "https://qoder.com/api/v2/me/usages/big_model_credits";
const CSRF_ECHO_TOKEN: &str = "_echo_csrf_using_sec_fetch_site_";
const BAXIA_SDK_VERSION: &str = "2.5.35";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36 Edg/149.0.0.0";
const USAGE_PAGE_REFERER: &str = "https://qoder.com/account/usage";

#[derive(Clone, Debug)]
pub(crate) struct QoderCredentials {
    pub(crate) session_cookie: String,
    source: String,
}

pub(crate) struct QoderQuotaAdapter;

impl ProviderQuotaAdapter for QoderQuotaAdapter {
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
            let credentials = match credential_source.qoder_credentials(app_config) {
                Ok(Some(credentials)) => credentials,
                Ok(None) => {
                    return no_creds_snapshot();
                }
                Err(error) => {
                    return failed_snapshot("manual qoder session cookie", Some(error.to_string()));
                }
            };

            let response = usage_transport
                .qoder_usage(&credentials.session_cookie)
                .await;
            derive_snapshot(credentials, response)
        })
    }
}

pub fn qoder_quota_from_response(provider_id: &str, body: &str) -> Option<ProviderQuotaSnapshot> {
    let response: QoderUsageResponse = serde_json::from_str(body).ok()?;
    snapshot_from_response(provider_id, &response)
}

fn snapshot_from_response(
    provider_id: &str,
    response: &QoderUsageResponse,
) -> Option<ProviderQuotaSnapshot> {
    let plan_summary = response
        .plan_quota
        .as_ref()
        .and_then(|block| block.quota_summary.as_ref())?;

    let reset_at = response.next_reset_at.and_then(unix_millis_to_iso);

    let mut windows = Vec::new();

    windows.push(window("Monthly limit", plan_summary, reset_at.clone())?);
    if let Some(pack_window) = response
        .resource_package_quota
        .as_ref()
        .and_then(|block| block.quota_summary.as_ref())
        .filter(|summary| summary.limit_value.unwrap_or(0.0) > 0.0)
        .and_then(|summary| window("Resource pack", summary, reset_at))
    {
        windows.push(pack_window);
    }

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some("Qoder".to_string()),
        primary: None,
        windows,
        credential: None,
        error: None,
    })
}

fn derive_snapshot(
    credentials: QoderCredentials,
    response: Result<QoderUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match response {
        Ok(value) => match snapshot_from_response(PROVIDER_ID, &value) {
            Some(mut snapshot) => {
                snapshot.credential = Some(credentials.source);
                snapshot
            }
            None => failed_snapshot(
                &credentials.source,
                Some(
                    "Qoder personal quota response did not contain a plan_quota block".to_string(),
                ),
            ),
        },
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan: Some("Qoder".to_string()),
            primary: None,
            windows: Vec::new(),
            credential: Some(credentials.source),
            error: Some(
                "Qoder session cookie expired; copy a fresh one from qoder.com DevTools"
                    .to_string(),
            ),
        },
        Err(error) => failed_snapshot(&credentials.source, Some(error.to_string())),
    }
}

pub(crate) fn read_credentials(
    app_config: &AppConfigService,
) -> AppResult<Option<QoderCredentials>> {
    let params = app_config.get_qoder_connection_params()?;
    let trimmed = params.session_cookie.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(QoderCredentials {
        session_cookie: trimmed,
        source: "manual qoder session cookie".to_string(),
    }))
}

pub(crate) async fn fetch_usage(
    session_cookie: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<QoderUsageResponse, ProviderQuotaPollError> {
    let trimmed = session_cookie.trim();
    if trimmed.is_empty() {
        return Err(ProviderQuotaPollError::AuthRequired);
    }

    let response = request_logger
        .send(
            http_client()
                .get(QUOTA_URL)
                .header("Cookie", format!("qoder_session_cookie={trimmed}"))
                .header("bx-v", BAXIA_SDK_VERSION)
                .header("x-csrf-token", CSRF_ECHO_TOKEN)
                .header("x-requested-with", "XMLHttpRequest")
                .header("User-Agent", BROWSER_UA)
                .header("Referer", USAGE_PAGE_REFERER)
                .header("Accept", "application/json, text/plain, */*")
                .header("Sec-Fetch-Site", "same-origin")
                .header("Sec-Fetch-Mode", "cors")
                .header("Sec-Fetch-Dest", "empty"),
            provider_quota_log_context("qoder_usage", PROVIDER_ID, "GET", QUOTA_URL),
        )
        .await
        .map_err(provider_quota_request_error)?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Qoder quota endpoint returned {status}"
        )));
    }

    response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
        .and_then(|body| {
            serde_json::from_str::<QoderUsageResponse>(&body)
                .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
        })
}

fn window(
    label: &str,
    summary: &QoderQuotaSummary,
    reset_at: Option<String>,
) -> Option<ProviderQuotaWindow> {
    let used = summary
        .usage_percentage
        .or_else(|| usage_percentage(summary.used_value, summary.limit_value))
        .map(percent_to_u8)?;

    Some(ProviderQuotaWindow {
        label: label.to_string(),
        kind: ProviderQuotaWindowKind::Monthly,
        used,
        value_label: qoder_value_label(summary),
        value_only: false,
        reset_at,
        unlimited: false,
    })
}

fn usage_percentage(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    let limit = limit?;
    if limit <= 0.0 {
        return None;
    }
    Some(used.unwrap_or(0.0) / limit * 100.0)
}

fn qoder_value_label(summary: &QoderQuotaSummary) -> Option<String> {
    let used = summary.used_value?;
    let limit = summary.limit_value?;
    let unit = summary.unit.as_deref().unwrap_or("credits");
    Some(format!(
        "{} / {} {}",
        format_qoder_amount(used),
        format_qoder_amount(limit),
        unit,
    ))
}

fn format_qoder_amount(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format_integer_with_commas(value as i64)
    } else {
        let formatted = format!("{value:.2}");
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn format_integer_with_commas(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut formatted = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    let result: String = formatted.chars().rev().collect();
    if value < 0 {
        format!("-{result}")
    } else {
        result
    }
}

fn no_creds_snapshot() -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        status: ProviderQuotaStatus::NoCreds,
        plan: Some("Qoder".to_string()),
        primary: None,
        windows: Vec::new(),
        credential: Some("manual qoder session cookie".to_string()),
        error: None,
    }
}

fn failed_snapshot(credential: &str, error: Option<String>) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        status: ProviderQuotaStatus::Failed,
        plan: Some("Qoder".to_string()),
        primary: None,
        windows: Vec::new(),
        credential: Some(credential.to_string()),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_quota_with_used_and_limit_yields_monthly_window() {
        let body = r#"{
            "status": "active",
            "plan_quota": {
                "quota_summary": {
                    "used_value": 17,
                    "limit_value": 3000,
                    "remaining_value": 2983,
                    "usage_percentage": 1,
                    "unit": "credits"
                },
                "quota_detail": null
            }
        }"#;

        let snapshot =
            qoder_quota_from_response(PROVIDER_ID, body).expect("map plan quota to snapshot");

        assert_eq!(snapshot.provider_id, "qoder");
        assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
        assert_eq!(snapshot.plan.as_deref(), Some("Qoder"));
        assert_eq!(snapshot.primary, None);
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].label, "Monthly limit");
        assert_eq!(snapshot.windows[0].used, 1);
        assert_eq!(
            snapshot.windows[0].value_label.as_deref(),
            Some("17 / 3,000 credits"),
        );
        assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Monthly);
    }

    #[test]
    fn next_reset_at_unix_millis_maps_to_window_reset_iso() {
        let body = r#"{
            "status": "active",
            "nextResetAt": 1785545197000,
            "plan_quota": {
                "quota_summary": {
                    "used_value": 17,
                    "limit_value": 3000,
                    "usage_percentage": 1
                }
            }
        }"#;

        let snapshot = qoder_quota_from_response(PROVIDER_ID, body).expect("parse reset mapping");

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(
            snapshot.windows[0].reset_at.as_deref(),
            Some("2026-08-01T00:46:37Z"),
        );
    }

    #[test]
    fn resource_package_with_nonzero_limit_appends_second_window() {
        let body = r#"{
            "status": "active",
            "plan_quota": {
                "quota_summary": {
                    "used_value": 17,
                    "limit_value": 3000,
                    "usage_percentage": 1
                }
            },
            "resource_package_quota": {
                "quota_summary": {
                    "used_value": 200,
                    "limit_value": 1000,
                    "usage_percentage": 20
                }
            }
        }"#;

        let snapshot = qoder_quota_from_response(PROVIDER_ID, body).expect("map both windows");

        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].label, "Monthly limit");
        assert_eq!(snapshot.windows[0].used, 1);
        assert_eq!(snapshot.windows[1].label, "Resource pack");
        assert_eq!(snapshot.windows[1].used, 20);
        assert_eq!(
            snapshot.windows[1].value_label.as_deref(),
            Some("200 / 1,000 credits"),
        );
        assert_eq!(snapshot.primary, None);
    }

    #[test]
    fn resource_package_with_zero_limit_is_omitted() {
        let body = r#"{
            "status": "active",
            "plan_quota": {
                "quota_summary": {
                    "used_value": 17,
                    "limit_value": 3000,
                    "usage_percentage": 1
                }
            },
            "resource_package_quota": {
                "quota_summary": {
                    "used_value": 0,
                    "limit_value": 0,
                    "usage_percentage": 0
                }
            }
        }"#;

        let snapshot =
            qoder_quota_from_response(PROVIDER_ID, body).expect("parse plan without empty pack");

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].label, "Monthly limit");
        assert_eq!(snapshot.primary, None);
    }

    #[test]
    fn top_level_restricted_status_does_not_invalidate_snapshot() {
        let body = r#"{
            "status": "restricted",
            "plan_quota": {
                "quota_summary": {
                    "used_value": 3000,
                    "limit_value": 3000,
                    "usage_percentage": 100
                }
            }
        }"#;

        let snapshot =
            qoder_quota_from_response(PROVIDER_ID, body).expect("restricted response still maps");

        assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].used, 100);
    }
}
