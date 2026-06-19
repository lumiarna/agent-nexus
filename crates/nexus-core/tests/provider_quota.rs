use nexus_core::services::provider_quota::{
    claude_code_quota_from_usage_response, codex_quota_from_usage_response,
    copilot_quota_from_usage_response, ClaudeCodeUsageBucket, ClaudeCodeUsageResponse,
    parse_opencode_copilot_token, CodexRateLimit, CodexRateLimitWindow, CodexUsageResponse,
    CopilotQuotaDetail, CopilotQuotaSnapshots, CopilotUsageResponse, ProviderQuotaStatus,
    ProviderQuotaWindowKind,
};

fn copilot_detail(percent_remaining: Option<f64>) -> CopilotQuotaDetail {
    CopilotQuotaDetail {
        entitlement: None,
        remaining: None,
        percent_remaining,
        unlimited: None,
    }
}

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
fn copilot_usage_endpoint_maps_premium_and_chat_windows() {
    let snapshot = copilot_quota_from_usage_response(
        "copilot",
        CopilotUsageResponse {
            copilot_plan: Some("business".to_string()),
            quota_reset_date: Some("2026-07-01".to_string()),
            quota_snapshots: Some(CopilotQuotaSnapshots {
                premium_interactions: Some(copilot_detail(Some(23.0))),
                chat: Some(copilot_detail(Some(80.0))),
            }),
        },
    );

    assert_eq!(snapshot.provider_id, "copilot");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Copilot Business"));
    // premium used = 100 - 23 = 77, chat used = 100 - 80 = 20; peak = 77.
    assert_eq!(snapshot.primary, Some(77));
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].label, "Premium Interactions");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[0].used, 77);
    assert_eq!(
        snapshot.windows[0].reset_at.as_deref(),
        Some("2026-07-01T00:00:00Z"),
    );
    assert_eq!(snapshot.windows[1].label, "Chat Quota");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[1].used, 20);
    assert_eq!(
        snapshot.windows[1].reset_at.as_deref(),
        Some("2026-07-01T00:00:00Z"),
    );
}

#[test]
fn copilot_usage_derives_remaining_from_entitlement_when_percent_absent() {
    let snapshot = copilot_quota_from_usage_response(
        "copilot",
        CopilotUsageResponse {
            copilot_plan: Some("copilot_pro".to_string()),
            quota_reset_date: Some("2026-07-01".to_string()),
            quota_snapshots: Some(CopilotQuotaSnapshots {
                premium_interactions: Some(CopilotQuotaDetail {
                    entitlement: Some(300),
                    remaining: Some(75.0),
                    percent_remaining: None,
                    unlimited: None,
                }),
                chat: None,
            }),
        },
    );

    assert_eq!(snapshot.plan.as_deref(), Some("Copilot Pro"));
    assert_eq!(snapshot.windows.len(), 1);
    assert_eq!(snapshot.windows[0].label, "Premium Interactions");
    // remaining 75 / entitlement 300 = 25% remaining => 75% used.
    assert_eq!(snapshot.windows[0].used, 75);
    assert_eq!(snapshot.primary, Some(75));
}

#[test]
fn copilot_usage_treats_unlimited_window_as_zero_used() {
    let snapshot = copilot_quota_from_usage_response(
        "copilot",
        CopilotUsageResponse {
            copilot_plan: Some("business".to_string()),
            quota_reset_date: Some("2026-07-01".to_string()),
            quota_snapshots: Some(CopilotQuotaSnapshots {
                premium_interactions: Some(copilot_detail(Some(40.0))),
                chat: Some(CopilotQuotaDetail {
                    entitlement: Some(0),
                    remaining: Some(0.0),
                    percent_remaining: None,
                    unlimited: Some(true),
                }),
            }),
        },
    );

    assert_eq!(snapshot.windows.len(), 2);
    assert!(!snapshot.windows[0].unlimited);
    assert_eq!(snapshot.windows[1].label, "Chat Quota");
    assert_eq!(snapshot.windows[1].used, 0);
    assert!(snapshot.windows[1].unlimited);
}

#[test]
fn copilot_usage_drops_windows_without_quota_data() {
    let snapshot = copilot_quota_from_usage_response(
        "copilot",
        CopilotUsageResponse {
            copilot_plan: None,
            quota_reset_date: None,
            quota_snapshots: Some(CopilotQuotaSnapshots {
                premium_interactions: None,
                chat: Some(copilot_detail(Some(60.0))),
            }),
        },
    );

    assert_eq!(snapshot.plan, None);
    assert_eq!(snapshot.windows.len(), 1);
    assert_eq!(snapshot.windows[0].label, "Chat Quota");
    assert_eq!(snapshot.windows[0].used, 40);
    assert_eq!(snapshot.windows[0].reset_at, None);
}

#[test]
fn opencode_auth_yields_github_copilot_access_token() {
    let content = r#"{
        "github-copilot": { "type": "oauth", "refresh": "gho_refresh", "access": "gho_access", "expires": 0 },
        "openrouter": { "type": "api", "key": "sk-or-xxx" }
    }"#;

    assert_eq!(
        parse_opencode_copilot_token(content).as_deref(),
        Some("gho_access"),
    );
}

#[test]
fn opencode_auth_without_github_copilot_entry_yields_none() {
    let content = r#"{ "openrouter": { "type": "api", "key": "sk-or-xxx" } }"#;
    assert_eq!(parse_opencode_copilot_token(content), None);
}

#[test]
fn opencode_auth_with_blank_token_yields_none() {
    let content = r#"{ "github-copilot": { "type": "oauth", "access": "  " } }"#;
    assert_eq!(parse_opencode_copilot_token(content), None);
}

#[test]
fn opencode_auth_invalid_json_yields_none() {
    assert_eq!(parse_opencode_copilot_token("not json"), None);
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
