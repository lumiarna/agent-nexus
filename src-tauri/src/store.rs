use std::sync::Arc;

use nexus_core::{
    database::Database,
    services::{
        app_config::AppConfigService, outbound_request_log::OutboundRequestLogger,
        project_symlinks::ProjectSymlinkInventory, projects::ProjectService,
        prompts::PromptService, provider_quota::ProviderQuotaService,
        provider_trigger::ProviderTriggerService, sessions::SessionService, skills::SkillService,
        sync::SyncService,
    },
};

pub struct AppState {
    pub app_config: AppConfigService,
    pub prompts: PromptService,
    pub projects: ProjectService,
    pub project_symlinks: ProjectSymlinkInventory,
    pub provider_quota: ProviderQuotaService,
    pub provider_trigger: ProviderTriggerService,
    pub sessions: SessionService,
    pub skills: SkillService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new(db: Database, request_logger: OutboundRequestLogger) -> Self {
        let db = Arc::new(db);
        let app_config = AppConfigService::new(db.clone());
        Self {
            app_config: app_config.clone(),
            prompts: PromptService::new(db.clone()),
            projects: ProjectService::new(db.clone()),
            project_symlinks: ProjectSymlinkInventory::new(db.clone()),
            provider_quota: ProviderQuotaService::new(app_config.clone(), request_logger.clone()),
            provider_trigger: ProviderTriggerService::new(
                db.clone(),
                AppConfigService::new(db.clone()),
                request_logger.clone(),
            ),
            sessions: SessionService::new(db.clone(), request_logger.clone()),
            skills: SkillService::new(db.clone(), app_config),
            sync: SyncService::new(db, request_logger),
        }
    }
}
