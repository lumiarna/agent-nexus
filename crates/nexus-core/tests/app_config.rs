use std::{path::PathBuf, sync::Arc};

use nexus_core::{
    database::Database,
    services::app_config::{AppConfigService, CLAUDE_CONFIG_DIR_KEY},
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
