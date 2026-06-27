use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use time::{format_description::well_known::Rfc3339, OffsetDateTime, Time};

use crate::services::outbound_request_log::{OutboundRequestContext, OutboundRequestError};

use super::{
    ProviderQuotaPollError, ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow,
    ProviderQuotaWindowKind,
};

pub(crate) fn unix_seconds_to_iso(secs: i64) -> Option<String> {
    OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
}

pub(crate) fn unix_millis_to_iso(ms: i64) -> Option<String> {
    if ms <= 0 {
        return None;
    }
    unix_seconds_to_iso(ms / 1000)
}

pub(crate) fn reset_seconds_to_iso(now_epoch_seconds: i64, reset_in_sec: u64) -> Option<String> {
    if reset_in_sec == 0 {
        return None;
    }
    let reset_in_sec = i64::try_from(reset_in_sec).ok()?;
    unix_seconds_to_iso(now_epoch_seconds.checked_add(reset_in_sec)?)
}

pub(crate) fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn llm_gateway_quota_from_headers(
    provider_id: &str,
    plan: &str,
    headers: &[(&str, &str)],
) -> Option<ProviderQuotaSnapshot> {
    llm_gateway_quota_from_headers_at(provider_id, plan, headers, current_epoch_seconds())
}

pub fn llm_gateway_quota_from_headers_at(
    provider_id: &str,
    plan: &str,
    headers: &[(&str, &str)],
    now_epoch_seconds: i64,
) -> Option<ProviderQuotaSnapshot> {
    let values = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .trim()
                .parse::<u64>()
                .ok()
                .map(|parsed| (name.to_ascii_lowercase(), parsed))
        })
        .collect::<BTreeMap<_, _>>();

    let windows = [
        gateway_quota_window(
            &values,
            "Minute limit",
            ProviderQuotaWindowKind::Rolling,
            &["x-token-count-limit-per-minute"],
            "x-token-count-used-per-minute",
            None,
        ),
        gateway_quota_window(
            &values,
            "Hourly limit",
            ProviderQuotaWindowKind::Rolling,
            &[
                "x-token-count-limit-per-hour-and-user",
                "x-token-count-limit-per-hour-and-client-id",
            ],
            "x-token-count-used-per-hour",
            None,
        ),
        gateway_quota_window(
            &values,
            "Daily limit",
            ProviderQuotaWindowKind::Rolling,
            &[
                "x-token-count-limit-per-day-and-user",
                "x-token-count-limit-per-day-and-client-id",
            ],
            "x-token-count-used-per-day",
            None,
        ),
        gateway_quota_window(
            &values,
            "Monthly limit",
            ProviderQuotaWindowKind::Monthly,
            &[
                "x-token-count-limit-per-month-and-user",
                "x-token-count-limit-per-month-and-client-id",
            ],
            "x-token-count-used-per-month",
            next_natural_month_reset_at(now_epoch_seconds),
        ),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if windows.is_empty() {
        return None;
    }

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some(plan.to_string()),
        primary: windows.first().map(|window| window.used),
        windows,
        credential: None,
        error: None,
    })
}

fn gateway_quota_window(
    headers: &BTreeMap<String, u64>,
    label: &str,
    kind: ProviderQuotaWindowKind,
    limit_headers: &[&str],
    used_header: &str,
    reset_at: Option<String>,
) -> Option<ProviderQuotaWindow> {
    let limit = limit_headers
        .iter()
        .filter_map(|name| headers.get(*name).copied())
        .min()?;
    if limit == 0 {
        return None;
    }
    let used_tokens = headers.get(used_header).copied()?;

    Some(ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(used_tokens as f64 / limit as f64 * 100.0),
        value_label: Some(format!(
            "{} / {} tokens",
            format_token_count(used_tokens),
            format_token_count(limit)
        )),
        value_only: false,
        reset_at,
        unlimited: false,
    })
}

fn next_natural_month_reset_at(now_epoch_seconds: i64) -> Option<String> {
    let current_month_start = OffsetDateTime::from_unix_timestamp(now_epoch_seconds)
        .ok()?
        .replace_day(1)
        .ok()?
        .replace_time(Time::MIDNIGHT);
    let next_month = current_month_start
        .checked_add(time::Duration::days(32))?
        .replace_day(1)
        .ok()?;
    next_month.format(&Rfc3339).ok()
}

fn format_token_count(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
}

pub(crate) fn percent_to_u8(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, 100.0) as u8
}

pub(crate) fn shortest_percent_window_used(windows: &[ProviderQuotaWindow]) -> Option<u8> {
    let shortest_rank = windows
        .iter()
        .filter(|window| !window.value_only)
        .map(|window| quota_window_kind_rank(&window.kind))
        .min()?;

    windows
        .iter()
        .filter(|window| !window.value_only)
        .filter(|window| quota_window_kind_rank(&window.kind) == shortest_rank)
        .map(|window| window.used)
        .max()
}

fn quota_window_kind_rank(kind: &ProviderQuotaWindowKind) -> u8 {
    match kind {
        ProviderQuotaWindowKind::Rolling => 0,
        ProviderQuotaWindowKind::Weekly => 1,
        ProviderQuotaWindowKind::Monthly => 2,
    }
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build HTTP client")
}

pub(crate) fn provider_quota_log_context(
    operation: &'static str,
    provider_id: &str,
    method: &'static str,
    url: &str,
) -> OutboundRequestContext {
    OutboundRequestContext {
        category: "provider_quota",
        operation,
        provider_id: Some(provider_id.to_string()),
        method,
        url: url.to_string(),
    }
}

pub(crate) fn provider_quota_request_error(error: OutboundRequestError) -> ProviderQuotaPollError {
    ProviderQuotaPollError::Request(error.to_string())
}
