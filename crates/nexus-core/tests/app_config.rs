use std::{path::PathBuf, sync::Arc};

use nexus_core::{
    database::Database,
    services::app_config::{
        AppConfigService, CLAUDE_CONFIG_DIR_KEY, CODEX_CONFIG_DIR_KEY,
    },
};

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

    assert!(dir.ends_with(".codex"), "default should resolve to ~/.codex, got {dir:?}");
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
