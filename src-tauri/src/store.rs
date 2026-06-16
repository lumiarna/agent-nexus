use std::sync::Arc;

use crate::{database::Database, services::projects::ProjectService};

pub struct AppState {
    pub projects: ProjectService,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        Self {
            projects: ProjectService::new(db),
        }
    }
}
