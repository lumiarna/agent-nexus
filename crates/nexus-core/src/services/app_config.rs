use std::{path::PathBuf, sync::Arc};

use rusqlite::{params, OptionalExtension};

use crate::{
    database::Database,
    error::AppResult,
    services::paths::{path_to_string, resolve_local_path},
};

pub const CLAUDE_CONFIG_DIR_KEY: &str = "CLAUDE_CONFIG_DIR";
const DEFAULT_CLAUDE_CONFIG_DIR: &str = "~/.claude";

pub const CODEX_CONFIG_DIR_KEY: &str = "CODEX_CONFIG_DIR";
const DEFAULT_CODEX_CONFIG_DIR: &str = "~/.codex";

#[derive(Clone)]
pub struct AppConfigService {
    db: Arc<Database>,
}

impl AppConfigService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn get_claude_config_dir(&self) -> AppResult<PathBuf> {
        let raw = self
            .read_setting(CLAUDE_CONFIG_DIR_KEY)?
            .unwrap_or_else(|| DEFAULT_CLAUDE_CONFIG_DIR.to_string());
        resolve_local_path(&raw)
    }

    pub fn get_claude_config_dir_display(&self) -> AppResult<String> {
        path_to_string(&self.get_claude_config_dir()?, "Claude config dir")
    }

    pub fn get_codex_config_dir(&self) -> AppResult<PathBuf> {
        let raw = self
            .read_setting(CODEX_CONFIG_DIR_KEY)?
            .unwrap_or_else(|| DEFAULT_CODEX_CONFIG_DIR.to_string());
        resolve_local_path(&raw)
    }

    pub fn get_codex_config_dir_display(&self) -> AppResult<String> {
        path_to_string(&self.get_codex_config_dir()?, "Codex config dir")
    }

    fn read_setting(&self, key: &str) -> AppResult<Option<String>> {
        let conn = self.db.connection()?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }
}
