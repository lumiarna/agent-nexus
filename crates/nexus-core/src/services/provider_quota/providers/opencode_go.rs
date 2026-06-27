use serde::Deserialize;

use crate::{
    error::AppResult,
    services::{
        app_config::{AppConfigService, OpenCodeGoConnectionParams},
        outbound_request_log::OutboundRequestLogger,
    },
};

use super::super::{
    shared::{
        current_epoch_seconds, http_client, percent_to_u8, provider_quota_log_context,
        provider_quota_request_error, reset_seconds_to_iso, shortest_percent_window_used,
    },
    ProviderCredentialSource, ProviderQuotaAdapter, ProviderQuotaFuture, ProviderQuotaPollError,
    ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow, ProviderQuotaWindowKind,
    ProviderUsageTransport,
};

pub(crate) const PROVIDER_ID: &str = "opencode-go";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone, Debug)]
pub(crate) struct OpenCodeGoCredentials {
    pub(crate) workspace_id: String,
    pub(crate) auth_cookie: String,
    source: String,
}

pub(crate) struct OpenCodeGoQuotaAdapter;

impl ProviderQuotaAdapter for OpenCodeGoQuotaAdapter {
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
            let credentials = match credential_source.opencode_go_credentials(app_config) {
                Ok(Some(credentials)) => credentials,
                Ok(None) => {
                    return status(
                        ProviderQuotaStatus::NoCreds,
                        "manual workspace id + auth cookie",
                        None,
                    );
                }
                Err(error) => {
                    return status(
                        ProviderQuotaStatus::Failed,
                        "manual workspace id + auth cookie",
                        Some(error.to_string()),
                    );
                }
            };

            let html = usage_transport
                .opencode_go_page(&credentials.workspace_id, &credentials.auth_cookie)
                .await;
            derive_snapshot(credentials, html)
        })
    }
}

pub fn opencode_go_quota_from_html(
    provider_id: &str,
    html: &str,
    now_epoch_seconds: i64,
) -> Option<ProviderQuotaSnapshot> {
    let rolling = extract_opencode_go_window(html, "rollingUsage");
    let weekly = extract_opencode_go_window(html, "weeklyUsage");
    let monthly = extract_opencode_go_window(html, "monthlyUsage");

    if rolling.is_none() && weekly.is_none() && monthly.is_none() {
        return None;
    }

    let mut windows = Vec::new();
    if let Some(window) = &rolling {
        windows.push(opencode_go_window(
            "Rolling (5h)",
            ProviderQuotaWindowKind::Rolling,
            window,
            now_epoch_seconds,
        ));
    }
    if let Some(window) = &weekly {
        windows.push(opencode_go_window(
            "Weekly limit",
            ProviderQuotaWindowKind::Weekly,
            window,
            now_epoch_seconds,
        ));
    }
    if let Some(window) = &monthly {
        windows.push(opencode_go_window(
            "Monthly limit",
            ProviderQuotaWindowKind::Monthly,
            window,
            now_epoch_seconds,
        ));
    }

    let primary = shortest_percent_window_used(&windows);
    let plan =
        Some(extract_string_field(html, "subscriptionPlan").unwrap_or_else(|| "Go".to_string()));

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    })
}

fn derive_snapshot(
    credentials: OpenCodeGoCredentials,
    page: Result<String, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match page {
        Ok(html) => {
            let Some(mut snapshot) =
                opencode_go_quota_from_html(PROVIDER_ID, &html, current_epoch_seconds())
            else {
                return ProviderQuotaSnapshot {
                    provider_id: PROVIDER_ID.to_string(),
                    status: ProviderQuotaStatus::Failed,
                    plan: Some("Go".to_string()),
                    primary: None,
                    windows: Vec::new(),
                    credential: Some(credentials.source),
                    error: Some(
                        "OpenCode Go page did not contain recognizable usage data".to_string(),
                    ),
                };
            };
            snapshot.credential = Some(credentials.source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan: Some("Go".to_string()),
            primary: None,
            windows: Vec::new(),
            credential: Some(credentials.source),
            error: Some(
                "OpenCode Go auth cookie expired; copy a fresh auth cookie from opencode.ai"
                    .to_string(),
            ),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan: Some("Go".to_string()),
            primary: None,
            windows: Vec::new(),
            credential: Some(credentials.source),
            error: Some(error.to_string()),
        },
    }
}

#[derive(Debug, Deserialize)]
struct OpenCodeGoWindow {
    usage_percent: f64,
    reset_in_sec: u64,
}

fn opencode_go_window(
    label: &str,
    kind: ProviderQuotaWindowKind,
    window: &OpenCodeGoWindow,
    now_epoch_seconds: i64,
) -> ProviderQuotaWindow {
    ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(window.usage_percent),
        value_label: None,
        value_only: false,
        reset_at: reset_seconds_to_iso(now_epoch_seconds, window.reset_in_sec),
        unlimited: false,
    }
}

fn extract_opencode_go_window(html: &str, key: &str) -> Option<OpenCodeGoWindow> {
    let obj = object_after_key(html, key)?;
    let usage_percent = number_field(obj, "usagePercent")?;
    let reset_in_sec = number_field(obj, "resetInSec").unwrap_or(0.0);
    Some(OpenCodeGoWindow {
        usage_percent,
        reset_in_sec: reset_in_sec.max(0.0) as u64,
    })
}

fn object_after_key<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}:");
    let mut from = 0;
    while let Some(rel) = s[from..].find(&needle) {
        let after = from + rel + needle.len();
        let rest = &s[after..];
        match rest.chars().next() {
            Some('{') | Some('$') => {
                let open = rest.find('{')?;
                let after_open = &rest[open + 1..];
                let close = after_open.find('}')?;
                return Some(&after_open[..close]);
            }
            _ => from = after,
        }
    }
    None
}

fn number_field(obj: &str, field: &str) -> Option<f64> {
    let needle = format!("{field}:");
    let idx = obj.find(&needle)? + needle.len();
    let token: String = obj[idx..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    token.parse().ok()
}

fn extract_string_field(s: &str, field: &str) -> Option<String> {
    let needle = format!("{field}:");
    let idx = s.find(&needle)? + needle.len();
    let tail = &s[idx..];
    let inner = tail.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

pub(crate) fn read_credentials(
    app_config: &AppConfigService,
) -> AppResult<Option<OpenCodeGoCredentials>> {
    let OpenCodeGoConnectionParams {
        workspace_id,
        auth_cookie,
    } = app_config.get_opencode_go_connection_params()?;
    if workspace_id.is_empty() || auth_cookie.is_empty() {
        return Ok(None);
    }
    Ok(Some(OpenCodeGoCredentials {
        workspace_id,
        auth_cookie,
        source: "manual workspace id + auth cookie".to_string(),
    }))
}

pub(crate) async fn fetch_page(
    workspace_id: &str,
    auth_cookie: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<String, ProviderQuotaPollError> {
    let id = normalize_workspace_id(workspace_id);
    let url = format!("https://opencode.ai/workspace/{id}/go");
    let response = request_logger
        .send(
            http_client()
                .get(&url)
                .header("Cookie", format!("auth={}", auth_cookie.trim()))
                .header("User-Agent", BROWSER_UA)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                ),
            provider_quota_log_context("opencode_go_page", PROVIDER_ID, "GET", &url),
        )
        .await
        .map_err(provider_quota_request_error)?;

    let final_url = response.url().as_str().to_string();
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if final_url.contains("auth.opencode.ai")
        || final_url.contains("/authorize")
        || final_url.contains("/login")
    {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "OpenCode Go workspace page returned {status}"
        )));
    }

    response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn normalize_workspace_id(workspace_id: &str) -> String {
    let workspace_id = workspace_id.trim();
    if workspace_id.starts_with("wrk_") {
        workspace_id.to_string()
    } else {
        format!("wrk_{workspace_id}")
    }
}

fn status(
    status: ProviderQuotaStatus,
    credential: &str,
    error: Option<String>,
) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: PROVIDER_ID.to_string(),
        status,
        plan: Some("Go".to_string()),
        primary: None,
        windows: Vec::new(),
        credential: Some(credential.to_string()),
        error,
    }
}
