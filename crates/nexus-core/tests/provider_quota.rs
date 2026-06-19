use nexus_core::services::provider_quota::{
    claude_code_quota_from_usage_response, codex_quota_from_usage_response, ClaudeCodeUsageBucket,
    ClaudeCodeUsageResponse, CodexRateLimit, CodexRateLimitWindow, CodexUsageResponse,
    ProviderQuotaStatus, ProviderQuotaWindowKind,
};

#[test]
fn claude_code_usage_endpoint_maps_to_provider_quota_snapshot() {
    let snapshot = claude_code_quota_from_usage_response(
        "claude",
        Some("Claude Pro".to_string()),
        ClaudeCodeUsageResponse {
            five_hour: Some(ClaudeCodeUsageBucket {
                utilization: 0.0,
                resets_at: Some("2026-06-19T06:59:00Z".to_string()),
            }),
            seven_day: Some(ClaudeCodeUsageBucket {
                utilization: 59.0,
                resets_at: Some("2026-06-21T07:00:00Z".to_string()),
            }),
        },
    );

    assert_eq!(snapshot.provider_id, "claude");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Claude Pro"));
    assert_eq!(snapshot.primary, Some(59));
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].label, "5-hour limit");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Rolling);
    assert_eq!(snapshot.windows[0].used, 0);
    assert_eq!(
        snapshot.windows[0].reset_at.as_deref(),
        Some("2026-06-19T06:59:00Z"),
    );
    assert_eq!(snapshot.windows[1].label, "Weekly limit");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Weekly);
    assert_eq!(snapshot.windows[1].used, 59);
    assert_eq!(
        snapshot.windows[1].reset_at.as_deref(),
        Some("2026-06-21T07:00:00Z"),
    );
}

#[test]
fn codex_usage_endpoint_maps_to_provider_quota_snapshot() {
    let snapshot = codex_quota_from_usage_response(
        "codex",
        Some("ChatGPT Plus".to_string()),
        CodexUsageResponse {
            plan_type: None,
            rate_limit: Some(CodexRateLimit {
                primary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(23.0),
                    limit_window_seconds: Some(18000),
                    reset_at: Some(1781269400),
                }),
                secondary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(51.0),
                    limit_window_seconds: Some(604800),
                    reset_at: Some(1781787800),
                }),
            }),
        },
    );

    assert_eq!(snapshot.provider_id, "codex");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("ChatGPT Plus"));
    assert_eq!(snapshot.primary, Some(51));
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].label, "5-hour limit");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Rolling);
    assert_eq!(snapshot.windows[0].used, 23);
    assert_eq!(
        snapshot.windows[0].reset_at.as_deref(),
        Some("2026-06-12T13:03:20Z"),
    );
    assert_eq!(snapshot.windows[1].label, "Weekly limit");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Weekly);
    assert_eq!(snapshot.windows[1].used, 51);
    assert_eq!(
        snapshot.windows[1].reset_at.as_deref(),
        Some("2026-06-18T13:03:20Z"),
    );
}

#[test]
fn codex_usage_with_only_primary_window_yields_single_window() {
    let snapshot = codex_quota_from_usage_response(
        "codex",
        None,
        CodexUsageResponse {
            plan_type: None,
            rate_limit: Some(CodexRateLimit {
                primary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(40.0),
                    limit_window_seconds: Some(18000),
                    reset_at: Some(1781269400),
                }),
                secondary_window: None,
            }),
        },
    );

    assert_eq!(snapshot.windows.len(), 1);
    assert_eq!(snapshot.windows[0].label, "5-hour limit");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Rolling);
    assert_eq!(snapshot.windows[0].used, 40);
    assert_eq!(snapshot.primary, Some(40));
}

#[test]
fn codex_usage_drops_windows_without_used_percent() {
    let snapshot = codex_quota_from_usage_response(
        "codex",
        None,
        CodexUsageResponse {
            plan_type: None,
            rate_limit: Some(CodexRateLimit {
                primary_window: Some(CodexRateLimitWindow {
                    used_percent: None,
                    limit_window_seconds: Some(18000),
                    reset_at: Some(1781269400),
                }),
                secondary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(70.0),
                    limit_window_seconds: Some(604800),
                    reset_at: None,
                }),
            }),
        },
    );

    assert_eq!(snapshot.windows.len(), 1);
    assert_eq!(snapshot.windows[0].label, "Weekly limit");
    assert_eq!(snapshot.windows[0].used, 70);
    assert_eq!(snapshot.primary, Some(70));
    assert_eq!(snapshot.windows[0].reset_at, None);
}

#[test]
fn codex_usage_response_plan_type_overrides_credential_plan() {
    let snapshot = codex_quota_from_usage_response(
        "codex",
        Some("ChatGPT Pro".to_string()),
        CodexUsageResponse {
            plan_type: Some("plus".to_string()),
            rate_limit: Some(CodexRateLimit {
                primary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(30.0),
                    limit_window_seconds: Some(18000),
                    reset_at: Some(1781269400),
                }),
                secondary_window: None,
            }),
        },
    );

    assert_eq!(
        snapshot.plan.as_deref(),
        Some("ChatGPT Plus"),
        "response plan_type should override credential plan",
    );
}

#[test]
fn codex_usage_response_without_plan_type_keeps_credential_plan() {
    let snapshot = codex_quota_from_usage_response(
        "codex",
        Some("ChatGPT Pro".to_string()),
        CodexUsageResponse {
            plan_type: None,
            rate_limit: Some(CodexRateLimit {
                primary_window: Some(CodexRateLimitWindow {
                    used_percent: Some(30.0),
                    limit_window_seconds: Some(18000),
                    reset_at: Some(1781269400),
                }),
                secondary_window: None,
            }),
        },
    );

    assert_eq!(
        snapshot.plan.as_deref(),
        Some("ChatGPT Pro"),
        "credential plan should be kept when response has no plan_type",
    );
}
