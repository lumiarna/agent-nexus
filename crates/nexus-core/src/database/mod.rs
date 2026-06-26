mod schema;

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub(crate) use schema::{DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS, DEFAULT_PROJECT_SYMLINK_MAX_DEPTH};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        Self::initialize(conn)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::initialize(conn)
    }

    fn initialize(conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn connection(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Internal("database lock poisoned".to_string()))
    }
}
