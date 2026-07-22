use std::{env, fs, sync::Arc};

use nexus_core::{
    database::Database,
    services::{
        app_config::{AppConfigService, CODEX_CONFIG_DIR_KEY},
        provider_trigger::{ProviderScheduleSettings, ProviderTriggerService},
    },
};
use serial_test::serial;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let previous = env::var_os(key);
        env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

#[tokio::test]
#[serial]
async fn codex_models_are_supported_when_auth_exists() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let codex_dir = temp_dir.path().join("codex-config");
    fs::create_dir_all(&codex_dir).expect("create codex dir");
    fs::write(
        codex_dir.join("auth.json"),
        r#"{"tokens":{"access_token":"token-1","account_id":"acct-1"}}"#,
    )
    .expect("write auth.json");

    let _guard = EnvVarGuard::set_path(CODEX_CONFIG_DIR_KEY, &codex_dir);

    let db = Arc::new(Database::open_in_memory().expect("open db"));
    let app_config = AppConfigService::new(db.clone());
    let service = ProviderTriggerService::new(
        db,
        app_config,
        nexus_core::services::outbound_request_log::OutboundRequestLogger::for_test()
            .expect("request logger"),
    );

    let capability = service
        .list_provider_trigger_models("codex")
        .await
        .expect("should succeed when auth exists");

    assert!(capability.supported, "codex should be supported with auth");
    assert!(
        !capability.models.is_empty(),
        "expected at least one CodeX model"
    );
}

#[test]
fn codex_schedule_can_be_saved_with_window_alignment_fields() {
    let db = Arc::new(Database::open_in_memory().expect("open db"));
    let app_config = AppConfigService::new(db.clone());
    let service = ProviderTriggerService::new(
        db,
        app_config,
        nexus_core::services::outbound_request_log::OutboundRequestLogger::for_test()
            .expect("request logger"),
    );

    let saved = service
        .set_provider_schedule_settings(
            "codex",
            ProviderScheduleSettings {
                quota_refresh_minutes: 5,
                window_align_cron: "30 8 * * *".to_string(),
                window_align_model_id: Some("codex-mini".to_string()),
                window_align_next_attempt_at: None,
                window_align_last_attempt_at: None,
                window_align_last_status: "never".to_string(),
                window_align_last_error: None,
            },
        )
        .expect("save codex schedule");

    assert_eq!(saved.window_align_cron, "30 8 * * *");
    assert_eq!(saved.window_align_model_id.as_deref(), Some("codex-mini"));
    assert!(saved.window_align_next_attempt_at.is_some());
}
