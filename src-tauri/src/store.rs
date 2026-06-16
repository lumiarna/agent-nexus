use std::sync::Arc;

use crate::{
    database::Database,
    services::{projects::ProjectService, sync::SyncService},
};

pub struct AppState {
    pub projects: ProjectService,
    pub sync: SyncService,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        Self {
            projects: ProjectService::new(db.clone()),
            sync: SyncService::new(db),
        }
    }
}
