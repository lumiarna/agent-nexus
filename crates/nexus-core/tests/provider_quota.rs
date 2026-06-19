use nexus_core::services::provider_quota::{
    claude_code_quota_from_usage_response, ClaudeCodeUsageBucket, ClaudeCodeUsageResponse,
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
