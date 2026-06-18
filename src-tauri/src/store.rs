use std::sync::Arc;

use nexus_core::{
    database::Database,
    services::{projects::ProjectService, skills::SkillService, sync::SyncService},
};

pub struct AppState {
    pub projects: ProjectService,
    pub skills: SkillService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        Self {
            projects: ProjectService::new(db.clone()),
            skills: SkillService::new(db.clone()),
            sync: SyncService::new(db),
        }
    }
}
