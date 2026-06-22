use std::{path::PathBuf, sync::Arc};

use nexus_core::{
    database::Database,
    services::app_config::{
        AppConfigService, OpenCodeGoConnectionParams, CLAUDE_CONFIG_DIR_KEY, CODEX_CONFIG_DIR_KEY,
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
