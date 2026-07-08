use std::{path::PathBuf, sync::Arc};

use nexus_core::{
    database::Database,
    services::app_config::{
        AgentDisplayPreferences, AppConfigService, OpenCodeGoConnectionParams,
        ProviderConnectionParams, ProviderDisplayPreferences, TrayMetric, CLAUDE_CONFIG_DIR_KEY,
        CODEX_CONFIG_DIR_KEY,
    },
};

#[test]
fn copilot_github_token_round_trips_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_copilot_github_token()
            .expect("read default copilot token"),
        Some(String::new()),
    );

    service
        .set_copilot_github_token("  gho_token  ")
        .expect("save copilot token");

    assert_eq!(
        service
            .get_copilot_github_token()
            .expect("read saved copilot token"),
        Some("gho_token".to_string()),
    );
}

#[test]
fn opencode_go_connection_params_round_trip_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_opencode_go_connection_params()
            .expect("read default OpenCode Go params"),
        OpenCodeGoConnectionParams::default(),
    );

    service
        .set_opencode_go_connection_params(&OpenCodeGoConnectionParams {
            workspace_id: "  wrk_abc  ".to_string(),
            auth_cookie: "  Fe26.2**cookie  ".to_string(),
        })
        .expect("save OpenCode Go params");

    assert_eq!(
        service
            .get_opencode_go_connection_params()
            .expect("read saved OpenCode Go params"),
        OpenCodeGoConnectionParams {
            workspace_id: "wrk_abc".to_string(),
            auth_cookie: "Fe26.2**cookie".to_string(),
        },
    );
}

#[test]
fn provider_connection_params_round_trip_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_provider_connection_params("deepseek")
            .expect("read default DeepSeek params"),
        ProviderConnectionParams::default(),
    );

    service
        .set_provider_connection_params(
            "deepseek",
            &ProviderConnectionParams {
                api_key: "  sk-deepseek  ".to_string(),
            },
        )
        .expect("save DeepSeek params");

    assert_eq!(
        service
            .get_provider_connection_params("deepseek")
            .expect("read saved DeepSeek params"),
        ProviderConnectionParams {
            api_key: "sk-deepseek".to_string(),
        },
    );
}

#[test]
fn provider_order_round_trips_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_provider_order()
            .expect("read default provider order"),
        Vec::<String>::new(),
    );

    assert_eq!(
        service
            .set_provider_order(&[
                "copilot".to_string(),
                "claude".to_string(),
                "opencode-go".to_string(),
            ])
            .expect("save provider order"),
        vec![
            "copilot".to_string(),
            "claude".to_string(),
            "opencode-go".to_string(),
        ],
    );

    assert_eq!(
        service
            .get_provider_order()
            .expect("read saved provider order"),
        vec![
            "copilot".to_string(),
            "claude".to_string(),
            "opencode-go".to_string(),
        ],
    );
}

#[test]
fn provider_display_preferences_round_trip_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_provider_display_preferences()
            .expect("read default provider display preferences"),
        ProviderDisplayPreferences {
            card_visibility: Vec::new(),
            tray_metric: TrayMetric::Remaining,
            tray_visibility: Vec::new(),
        },
    );

    assert_eq!(
        service
            .set_provider_display_preferences(&ProviderDisplayPreferences {
                card_visibility: vec!["copilot".to_string(), "claude".to_string()],
                tray_metric: TrayMetric::Used,
                tray_visibility: vec!["claude".to_string()],
            })
            .expect("save provider display preferences"),
        ProviderDisplayPreferences {
            card_visibility: vec!["copilot".to_string(), "claude".to_string()],
            tray_metric: TrayMetric::Used,
            tray_visibility: vec!["claude".to_string()],
        },
    );

    assert_eq!(
        service
            .get_provider_display_preferences()
            .expect("read saved provider display preferences"),
        ProviderDisplayPreferences {
            card_visibility: vec!["copilot".to_string(), "claude".to_string()],
            tray_metric: TrayMetric::Used,
            tray_visibility: vec!["claude".to_string()],
        },
    );
}

#[test]
fn provider_display_preferences_defaults_tray_metric_for_legacy_rows() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    {
        let conn = db.connection().expect("open db connection");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [
                "PROVIDER_CARD_VISIBILITY",
                r#"{"cardVisibility":["copilot","claude"]}"#,
            ],
        )
        .expect("write legacy provider display preferences");
    }

    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_provider_display_preferences()
            .expect("read legacy provider display preferences"),
        ProviderDisplayPreferences {
            card_visibility: vec!["copilot".to_string(), "claude".to_string()],
            tray_metric: TrayMetric::Remaining,
            tray_visibility: Vec::new(),
        },
    );
}

#[test]
fn agent_display_preferences_round_trip_through_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_agent_display_preferences()
            .expect("read default agent display preferences"),
        AgentDisplayPreferences::default(),
    );

    assert_eq!(
        service
            .set_agent_display_preferences(&AgentDisplayPreferences {
                disabled: vec!["Copilot".to_string(), "OpenCode".to_string()],
                default_global_entry_agent: Some("Claude Code".to_string()),
            })
            .expect("save agent display preferences"),
        AgentDisplayPreferences {
            disabled: vec!["Copilot".to_string(), "OpenCode".to_string()],
            default_global_entry_agent: Some("Claude Code".to_string()),
        },
    );

    assert_eq!(
        service
            .get_agent_display_preferences()
            .expect("read saved agent display preferences"),
        AgentDisplayPreferences {
            disabled: vec!["Copilot".to_string(), "OpenCode".to_string()],
            default_global_entry_agent: Some("Claude Code".to_string()),
        },
    );
}

#[test]
fn set_agent_display_preferences_rejects_disabling_generic_agent() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    let error = service
        .set_agent_display_preferences(&AgentDisplayPreferences {
            disabled: vec!["Generic Agent".to_string()],
            default_global_entry_agent: None,
        })
        .expect_err("Generic Agent must stay enabled");
    assert!(error.to_string().contains("Generic Agent cannot be disabled"));
}

#[test]
fn set_agent_display_preferences_rejects_unknown_agent() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    let error = service
        .set_agent_display_preferences(&AgentDisplayPreferences {
            disabled: vec!["claude".to_string()],
            default_global_entry_agent: None,
        })
        .expect_err("unknown agent name must be rejected");
    assert!(error.to_string().contains("unknown agent"));
}

#[test]
fn default_global_entry_agent_cleared_when_disabled() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    // A default that is also in the disabled list is cleared back to None,
    // never pointing at a hidden Agent.
    let saved = service
        .set_agent_display_preferences(&AgentDisplayPreferences {
            disabled: vec!["Copilot".to_string()],
            default_global_entry_agent: Some("Copilot".to_string()),
        })
        .expect("save agent display preferences");
    assert_eq!(saved.default_global_entry_agent, None);
}

#[test]
fn set_agent_display_preferences_rejects_unknown_default_entry() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    let error = service
        .set_agent_display_preferences(&AgentDisplayPreferences {
            disabled: vec![],
            default_global_entry_agent: Some("claude".to_string()),
        })
        .expect_err("unknown default entry agent must be rejected");
    assert!(error.to_string().contains("unknown agent"));
}

#[test]
fn reads_claude_config_dir_from_app_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    {
        let conn = db.connection().expect("open db connection");
        conn.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            ["/tmp/agent-nexus-claude", CLAUDE_CONFIG_DIR_KEY],
        )
        .expect("write Claude config dir setting");
    }

    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_claude_config_dir()
            .expect("read Claude config dir"),
        PathBuf::from("/tmp/agent-nexus-claude"),
    );
}

#[test]
fn codex_config_dir_defaults_to_dot_codex_when_unset() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = AppConfigService::new(db);

    let dir = service
        .get_codex_config_dir()
        .expect("read Codex config dir default");

    assert!(
        dir.ends_with(".codex"),
        "default should resolve to ~/.codex, got {dir:?}"
    );
}

#[test]
fn reads_codex_config_dir_from_app_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    {
        let conn = db.connection().expect("open db connection");
        conn.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            ["/tmp/agent-nexus-codex", CODEX_CONFIG_DIR_KEY],
        )
        .expect("write Codex config dir setting");
    }

    let service = AppConfigService::new(db);

    assert_eq!(
        service
            .get_codex_config_dir()
            .expect("read Codex config dir"),
        PathBuf::from("/tmp/agent-nexus-codex"),
    );
}
