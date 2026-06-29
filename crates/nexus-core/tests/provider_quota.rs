use std::{env, fs, sync::Arc};

use nexus_core::{
    database::Database,
    services::{
        app_config::{AppConfigService, CODEX_CONFIG_DIR_KEY},
        outbound_request_log::OutboundRequestLogger,
        provider_quota::{
            claude_code_quota_from_usage_response, codex_quota_from_usage_response,
            codex_reset_credit_windows, copilot_quota_from_usage_response,
            deepseek_balance_quota_from_usage_response, llm_gateway_quota_from_headers_at,
            minimax_token_plan_cn_quota_from_usage_response, opencode_go_quota_from_html,
            openrouter_credits_quota_from_usage_response, parse_opencode_copilot_token,
            parse_opencode_custom_providers, parse_opencode_provider_token, ClaudeCodeUsageBucket,
            ClaudeCodeUsageResponse, CodexRateLimit, CodexRateLimitWindow, CodexResetCredit,
            CodexResetCreditsResponse, CodexUsageResponse, CopilotQuotaDetail,
            CopilotQuotaSnapshots, CopilotUsageResponse, DeepSeekBalanceInfo,
            DeepSeekBalanceResponse, MiniMaxTokenPlanCnModelRemain,
            MiniMaxTokenPlanCnUsageResponse, OpenRouterCreditsData, OpenRouterCreditsResponse,
            ProviderQuotaService, ProviderQuotaStatus, ProviderQuotaWindowKind,
        },
    },
};
use serial_test::serial;

fn request_logger() -> OutboundRequestLogger {
    OutboundRequestLogger::for_test().expect("create request logger")
}

fn restore_env_var(key: &str, previous: Option<std::ffi::OsString>) {
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}

#[test]
fn opencode_config_exposes_custom_provider_metadata_without_credentials() {
    let providers = parse_opencode_custom_providers(
        r#"{
          "provider": {
            "llm-gateway-azure": {
              "name": "LLM Gateway Azure",
              "npm": "@ai-sdk/openai",
              "options": {
                "baseURL": "https://gateway.example/v2/openai/v1",
                "apiKey": "secret-must-not-leak"
              },
              "models": {
                "gpt-5.4": { "name": "GPT 5.4" }
              }
            }
          }
        }"#,
    )
    .expect("parse OpenCode custom providers");

    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "llm-gateway-azure");
    assert_eq!(providers[0].name, "LLM Gateway Azure");
    assert_eq!(providers[0].npm, "@ai-sdk/openai");
    assert_eq!(
        providers[0].base_url,
        "https://gateway.example/v2/openai/v1"
    );
    assert_eq!(providers[0].model_id, "gpt-5.4");
    assert!(!serde_json::to_string(&providers)
        .expect("serialize provider catalog")
        .contains("secret-must-not-leak"));
}

#[test]
fn llm_gateway_headers_map_effective_limits_to_quota_windows() {
    let snapshot = llm_gateway_quota_from_headers_at(
        "llm-gateway-alicloud",
        "OpenCode custom",
        &[
            ("x-token-count-limit-per-minute", "480000000"),
            ("x-token-count-used-per-minute", "0"),
            ("x-token-count-limit-per-hour-and-user", "60000000"),
            ("x-token-count-limit-per-hour-and-client-id", "480000000"),
            ("x-token-count-used-per-hour", "561685"),
            ("x-token-count-limit-per-day-and-user", "60000000"),
            ("x-token-count-limit-per-day-and-client-id", "480000000"),
            ("x-token-count-used-per-day", "693461"),
            ("x-token-count-limit-per-month-and-user", "60000000"),
            ("x-token-count-limit-per-month-and-client-id", "480000000"),
            ("x-token-count-used-per-month", "33608618"),
        ],
        1_782_309_600,
    )
    .expect("derive gateway quota");

    assert_eq!(snapshot.provider_id, "llm-gateway-alicloud");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.primary, Some(0));
    assert_eq!(snapshot.windows.len(), 4);
    assert_eq!(snapshot.windows[0].label, "Minute limit");
    assert_eq!(snapshot.windows[1].label, "Hourly limit");
    assert_eq!(snapshot.windows[1].used, 1);
    assert_eq!(
        snapshot.windows[1].value_label.as_deref(),
        Some("0.56m / 60m")
    );
    assert_eq!(snapshot.windows[3].label, "Monthly limit");
    assert_eq!(snapshot.windows[3].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[3].used, 56);
    assert_eq!(
        snapshot.windows[3].reset_at.as_deref(),
        Some("2026-07-01T00:00:00Z"),
    );
}

#[tokio::test]
#[serial]
async fn provider_quota_service_lists_and_dispatches_custom_provider_without_api_key() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let config_dir = temp_home.path().join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create OpenCode config dir");
    fs::write(
        config_dir.join("opencode.json"),
        r#"{
          "provider": {
            "custom-gateway": {
              "name": "Custom Gateway",
              "npm": "@ai-sdk/openai-compatible",
              "options": { "baseURL": "https://gateway.example/v1" },
              "models": { "test-model": {} }
            }
          }
        }"#,
    )
    .expect("write OpenCode config");
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", temp_home.path());

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let providers = service
        .list_opencode_custom_providers()
        .expect("list custom providers");
    let snapshot = service
        .get_provider_quota("custom-gateway")
        .await
        .expect("dispatch custom provider");

    restore_env_var("HOME", previous_home);

    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "custom-gateway");
    assert_eq!(snapshot.provider_id, "custom-gateway");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(
        snapshot.credential.as_deref(),
        Some("opencode.json · custom-gateway")
    );
}

#[tokio::test]
#[serial]
async fn provider_quota_service_ignores_opencode_config_env_overrides() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let config_dir = temp_home.path().join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create OpenCode config dir");
    fs::write(
        config_dir.join("opencode.json"),
        r#"{
          "provider": {
            "custom-gateway": {
              "name": "Custom Gateway",
              "npm": "@ai-sdk/openai-compatible",
              "options": { "baseURL": "https://gateway.example/v1" },
              "models": { "test-model": {} }
            }
          }
        }"#,
    )
    .expect("write default OpenCode config");
    let override_dir = temp_home.path().join("override");
    fs::create_dir_all(&override_dir).expect("create override dir");
    let override_file = override_dir.join("opencode.json");
    fs::write(
        &override_file,
        r#"{
          "provider": {
            "env-gateway": {
              "name": "Env Gateway",
              "npm": "@ai-sdk/openai-compatible",
              "options": { "baseURL": "https://env.example/v1" },
              "models": { "env-model": {} }
            }
          }
        }"#,
    )
    .expect("write env override OpenCode config");

    let previous_config_file = env::var_os("OPENCODE_CONFIG_FILE");
    let previous_config_dir = env::var_os("OPENCODE_CONFIG_DIR");
    let previous_home = env::var_os("HOME");
    env::set_var("OPENCODE_CONFIG_FILE", &override_file);
    env::set_var("OPENCODE_CONFIG_DIR", &override_dir);
    env::set_var("HOME", temp_home.path());

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let providers = service
        .list_opencode_custom_providers()
        .expect("list custom providers");

    restore_env_var("OPENCODE_CONFIG_FILE", previous_config_file);
    restore_env_var("OPENCODE_CONFIG_DIR", previous_config_dir);
    restore_env_var("HOME", previous_home);

    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "custom-gateway");
    assert!(!providers
        .iter()
        .any(|provider| provider.id == "env-gateway"));
}

fn copilot_detail(percent_remaining: Option<f64>) -> CopilotQuotaDetail {
    CopilotQuotaDetail {
        entitlement: None,
        remaining: None,
        percent_remaining,
        unlimited: None,
    }
}

#[tokio::test]
async fn provider_quota_service_dispatches_codex_adapter_without_credentials() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let missing_codex_dir = temp_dir.path().join("missing-codex");
    let missing_codex_dir = missing_codex_dir.to_string_lossy().into_owned();
    {
        let conn = db.connection().expect("open db connection");
        conn.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            [missing_codex_dir.as_str(), CODEX_CONFIG_DIR_KEY],
        )
        .expect("write Codex config dir setting");
    }

    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("codex")
        .await
        .expect("dispatch codex adapter");

    assert_eq!(snapshot.provider_id, "codex");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(snapshot.credential.as_deref(), Some("not found"));
    assert!(snapshot.windows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[tokio::test]
async fn provider_quota_service_dispatches_opencode_go_adapter_without_connection_params() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("opencode-go")
        .await
        .expect("dispatch opencode go adapter");

    assert_eq!(snapshot.provider_id, "opencode-go");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(
        snapshot.credential.as_deref(),
        Some("manual workspace id + auth cookie"),
    );
    assert!(snapshot.windows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[tokio::test]
#[serial]
async fn provider_quota_service_dispatches_minimax_token_plan_cn_adapter_without_credentials() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let temp_home = tempfile::tempdir().expect("create temp home");
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", temp_home.path());

    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("minimax-token")
        .await
        .expect("dispatch MiniMax Token Plan CN adapter");

    restore_env_var("HOME", previous_home);

    assert_eq!(snapshot.provider_id, "minimax-token");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(snapshot.credential.as_deref(), Some("not found"));
    assert!(snapshot.windows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[tokio::test]
#[serial]
async fn provider_quota_service_dispatches_deepseek_adapter_without_credentials() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let temp_home = tempfile::tempdir().expect("create temp home");
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", temp_home.path());

    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("deepseek")
        .await
        .expect("dispatch DeepSeek adapter");

    restore_env_var("HOME", previous_home);

    assert_eq!(snapshot.provider_id, "deepseek");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(snapshot.credential.as_deref(), Some("not found"));
    assert!(snapshot.windows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[tokio::test]
#[serial]
async fn provider_quota_service_dispatches_openrouter_adapter_without_credentials() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let temp_home = tempfile::tempdir().expect("create temp home");
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", temp_home.path());

    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("openrouter")
        .await
        .expect("dispatch OpenRouter adapter");

    restore_env_var("HOME", previous_home);

    assert_eq!(snapshot.provider_id, "openrouter");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(snapshot.credential.as_deref(), Some("not found"));
    assert!(snapshot.windows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[tokio::test]
#[serial]
async fn configured_provider_credentials_ignore_opencode_auth_file_env() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let temp_home = tempfile::tempdir().expect("create temp home");
    let override_dir = tempfile::tempdir().expect("create env override dir");
    let override_file = override_dir.path().join("auth.json");
    fs::write(
        &override_file,
        r#"{ "deepseek": { "type": "api", "key": "sk-env-should-be-ignored" } }"#,
    )
    .expect("write env override auth file");

    let previous_home = env::var_os("HOME");
    let previous_auth_file = env::var_os("OPENCODE_AUTH_FILE");
    env::set_var("HOME", temp_home.path());
    env::set_var("OPENCODE_AUTH_FILE", &override_file);

    let service = ProviderQuotaService::new(AppConfigService::new(db), request_logger());
    let snapshot = service
        .get_provider_quota("deepseek")
        .await
        .expect("dispatch DeepSeek adapter");

    restore_env_var("HOME", previous_home);
    restore_env_var("OPENCODE_AUTH_FILE", previous_auth_file);

    assert_eq!(snapshot.provider_id, "deepseek");
    assert_eq!(snapshot.status, ProviderQuotaStatus::NoCreds);
    assert_eq!(snapshot.credential.as_deref(), Some("not found"));
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
    assert_eq!(snapshot.primary, Some(0));
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
    assert_eq!(snapshot.primary, Some(23));
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
fn codex_reset_credits_map_available_credits_to_value_only_windows() {
    let windows = codex_reset_credit_windows(CodexResetCreditsResponse {
        credits: vec![
            CodexResetCredit {
                status: Some("available".to_string()),
                title: Some("Full reset (Weekly + 5 hr)".to_string()),
                expires_at: Some("2026-07-22T19:22:24.080059Z".to_string()),
            },
            CodexResetCredit {
                status: Some("available".to_string()),
                title: Some("Full reset (Weekly + 5 hr)".to_string()),
                expires_at: Some("2026-07-15T19:22:24.080059Z".to_string()),
            },
            CodexResetCredit {
                status: Some("redeemed".to_string()),
                title: Some("Already used".to_string()),
                expires_at: Some("2026-08-01T00:00:00Z".to_string()),
            },
        ],
    });

    // Redeemed credits are dropped; available ones are sorted soonest-expiry first.
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].label, "Full reset (Weekly + 5 hr)");
    assert_eq!(windows[0].value_label.as_deref(), Some("Expires Jul 15"));
    assert!(windows[0].value_only);
    assert_eq!(windows[0].used, 0);
    assert_eq!(windows[0].reset_at, None);
    assert_eq!(windows[1].value_label.as_deref(), Some("Expires Jul 22"));
}

#[test]
fn codex_reset_credits_fall_back_when_title_or_expiry_missing() {
    let windows = codex_reset_credit_windows(CodexResetCreditsResponse {
        credits: vec![CodexResetCredit {
            status: Some("available".to_string()),
            title: None,
            expires_at: None,
        }],
    });

    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].label, "Reset credit");
    assert_eq!(windows[0].value_label.as_deref(), Some("Available"));
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
    // Both windows are monthly, so the shortest-window display uses the higher monthly usage.
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
fn opencode_go_html_maps_to_provider_quota_snapshot() {
    let html = r#"
        <script>
        self.$R=[
          {subscriptionPlan:"Team"},
          {rollingUsage:{status:"ok",resetInSec:18000,usagePercent:23.4}},
          {weeklyUsage:$R[2]={status:"ok",resetInSec:604800,usagePercent:51}},
          {monthlyUsage:123,monthlyUsage:{status:"ok",resetInSec:2592000,usagePercent:34}}
        ];
        </script>
    "#;

    let snapshot = opencode_go_quota_from_html("opencode-go", html, 1_781_269_400)
        .expect("parse OpenCode Go usage from hydration HTML");

    assert_eq!(snapshot.provider_id, "opencode-go");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Team"));
    assert_eq!(snapshot.primary, Some(23));
    assert_eq!(snapshot.windows.len(), 3);
    assert_eq!(snapshot.windows[0].label, "Rolling (5h)");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Rolling);
    assert_eq!(snapshot.windows[0].used, 23);
    assert_eq!(
        snapshot.windows[0].reset_at.as_deref(),
        Some("2026-06-12T18:03:20Z"),
    );
    assert_eq!(snapshot.windows[1].label, "Weekly limit");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Weekly);
    assert_eq!(snapshot.windows[1].used, 51);
    assert_eq!(
        snapshot.windows[1].reset_at.as_deref(),
        Some("2026-06-19T13:03:20Z"),
    );
    assert_eq!(snapshot.windows[2].label, "Monthly limit");
    assert_eq!(snapshot.windows[2].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[2].used, 34);
    assert_eq!(
        snapshot.windows[2].reset_at.as_deref(),
        Some("2026-07-12T13:03:20Z"),
    );
}

#[test]
fn minimax_token_plan_cn_usage_maps_remaining_percent_to_used_windows() {
    let snapshot = minimax_token_plan_cn_quota_from_usage_response(
        "minimax-token",
        MiniMaxTokenPlanCnUsageResponse {
            model_remains: vec![
                MiniMaxTokenPlanCnModelRemain {
                    model_name: "video".to_string(),
                    end_time: None,
                    weekly_end_time: None,
                    current_interval_remaining_percent: Some(100.0),
                    current_weekly_remaining_percent: Some(100.0),
                    current_weekly_status: Some(3),
                },
                MiniMaxTokenPlanCnModelRemain {
                    model_name: "general".to_string(),
                    end_time: Some(1_781_269_400_000),
                    weekly_end_time: Some(1_781_787_800_000),
                    current_interval_remaining_percent: Some(98.0),
                    current_weekly_remaining_percent: Some(95.0),
                    current_weekly_status: Some(1),
                },
            ],
            base_resp: None,
        },
    )
    .expect("map MiniMax Token Plan CN usage");

    assert_eq!(snapshot.provider_id, "minimax-token");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Token plan"));
    assert_eq!(snapshot.primary, Some(2));
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].label, "5-hour limit");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Rolling);
    assert_eq!(snapshot.windows[0].used, 2);
    assert_eq!(
        snapshot.windows[0].reset_at.as_deref(),
        Some("2026-06-12T13:03:20Z"),
    );
    assert_eq!(snapshot.windows[1].label, "Weekly limit");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Weekly);
    assert_eq!(snapshot.windows[1].used, 5);
    assert_eq!(
        snapshot.windows[1].reset_at.as_deref(),
        Some("2026-06-18T13:03:20Z"),
    );
}

#[test]
fn deepseek_balance_response_maps_to_balance_window() {
    let snapshot = deepseek_balance_quota_from_usage_response(
        "deepseek",
        DeepSeekBalanceResponse {
            is_available: true,
            balance_infos: vec![DeepSeekBalanceInfo {
                currency: "CNY".to_string(),
                total_balance: "12.34".to_string(),
            }],
        },
    )
    .expect("map DeepSeek balance");

    assert_eq!(snapshot.provider_id, "deepseek");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Balance"));
    assert_eq!(snapshot.primary, None);
    assert_eq!(snapshot.windows.len(), 1);
    assert_eq!(snapshot.windows[0].label, "CNY balance");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[0].used, 0);
    assert_eq!(
        snapshot.windows[0].value_label.as_deref(),
        Some("12.34 CNY")
    );
    assert!(snapshot.windows[0].value_only);
}

#[test]
fn openrouter_credits_response_maps_to_credit_window() {
    let snapshot = openrouter_credits_quota_from_usage_response(
        "openrouter",
        OpenRouterCreditsResponse {
            data: OpenRouterCreditsData {
                total_credits: 100.50,
                total_usage: 25.75,
            },
        },
    )
    .expect("map OpenRouter credits");

    assert_eq!(snapshot.provider_id, "openrouter");
    assert_eq!(snapshot.status, ProviderQuotaStatus::Available);
    assert_eq!(snapshot.plan.as_deref(), Some("Credits"));
    assert_eq!(snapshot.primary, None);
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].label, "Credit used");
    assert_eq!(snapshot.windows[0].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[0].used, 0);
    assert_eq!(
        snapshot.windows[0].value_label.as_deref(),
        Some("25.75 credits used"),
    );
    assert!(snapshot.windows[0].value_only);
    assert_eq!(snapshot.windows[1].label, "Credit balance");
    assert_eq!(snapshot.windows[1].kind, ProviderQuotaWindowKind::Monthly);
    assert_eq!(snapshot.windows[1].used, 0);
    assert_eq!(
        snapshot.windows[1].value_label.as_deref(),
        Some("74.75 credits balance"),
    );
    assert!(snapshot.windows[1].value_only);
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
fn opencode_auth_yields_configured_provider_api_keys() {
    let content = r#"{
        "deepseek": { "type": "api", "key": "sk-deepseek" },
        "openrouter": { "type": "api", "key": "sk-or-xxx" }
    }"#;

    assert_eq!(
        parse_opencode_provider_token(content, "deepseek").as_deref(),
        Some("sk-deepseek"),
    );
    assert_eq!(
        parse_opencode_provider_token(content, "openrouter").as_deref(),
        Some("sk-or-xxx"),
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
