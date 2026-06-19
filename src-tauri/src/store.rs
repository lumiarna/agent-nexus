use std::sync::Arc;

use nexus_core::{
    database::Database,
    services::{
        app_config::AppConfigService, project_symlinks::ProjectSymlinkInventory,
        projects::ProjectService, provider_quota::ProviderQuotaService, sessions::SessionService,
        skills::SkillService, sync::SyncService,
    },
};

pub struct AppState {
    pub app_config: AppConfigService,
    pub projects: ProjectService,
    pub project_symlinks: ProjectSymlinkInventory,
    pub provider_quota: ProviderQuotaService,
    pub sessions: SessionService,
    pub skills: SkillService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        let app_config = AppConfigService::new(db.clone());
        Self {
            app_config: app_config.clone(),
            projects: ProjectService::new(db.clone()),
            project_symlinks: ProjectSymlinkInventory::new(db.clone()),
            provider_quota: ProviderQuotaService::new(app_config),
            sessions: SessionService::new(db.clone()),
            skills: SkillService::new(db.clone()),
            sync: SyncService::new(db),
        }
    }
}
