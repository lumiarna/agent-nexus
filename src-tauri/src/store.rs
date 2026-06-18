use std::sync::Arc;

use nexus_core::{
    database::Database,
    services::{
        project_symlinks::ProjectSymlinkInventory, projects::ProjectService,
        sessions::SessionService, skills::SkillService, sync::SyncService,
    },
};

pub struct AppState {
    pub projects: ProjectService,
    pub project_symlinks: ProjectSymlinkInventory,
    pub sessions: SessionService,
    pub skills: SkillService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        Self {
            projects: ProjectService::new(db.clone()),
            project_symlinks: ProjectSymlinkInventory::new(db.clone()),
            sessions: SessionService::new(db.clone()),
            skills: SkillService::new(db.clone()),
            sync: SyncService::new(db),
        }
    }
}
