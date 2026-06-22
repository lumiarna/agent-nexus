use std::{path::PathBuf, sync::Arc};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::paths::{path_to_string, resolve_local_path},
};

pub const CLAUDE_CONFIG_DIR_KEY: &str = "CLAUDE_CONFIG_DIR";
const DEFAULT_CLAUDE_CONFIG_DIR: &str = "~/.claude";

pub const CODEX_CONFIG_DIR_KEY: &str = "CODEX_CONFIG_DIR";
const DEFAULT_CODEX_CONFIG_DIR: &str = "~/.codex";

pub const COPILOT_GITHUB_TOKEN_KEY: &str = "COPILOT_GITHUB_TOKEN";
pub const OPENCODE_GO_WORKSPACE_ID_KEY: &str = "OPENCODE_GO_WORKSPACE_ID";
pub const OPENCODE_GO_AUTH_COOKIE_KEY: &str = "OPENCODE_GO_AUTH_COOKIE";
const MINIMAX_TOKEN_PLAN_CN_API_KEY_KEY: &str = "PROVIDER_API_KEY_MINIMAX_TOKEN";
const DEEPSEEK_API_KEY_KEY: &str = "PROVIDER_API_KEY_DEEPSEEK";
const OPENROUTER_API_KEY_KEY: &str = "PROVIDER_API_KEY_OPENROUTER";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoConnectionParams {
    pub workspace_id: String,
    pub auth_cookie: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionParams {
    pub api_key: String,
}

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

    pub fn get_copilot_github_token(&self) -> AppResult<Option<String>> {
        Ok(self
            .read_setting(COPILOT_GITHUB_TOKEN_KEY)?
            .map(|value| value.trim().to_string()))
    }

    pub fn set_copilot_github_token(&self, token: &str) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![COPILOT_GITHUB_TOKEN_KEY, token.trim()],
        )?;
        Ok(())
    }

    pub fn get_opencode_go_connection_params(&self) -> AppResult<OpenCodeGoConnectionParams> {
        Ok(OpenCodeGoConnectionParams {
            workspace_id: self
                .read_setting(OPENCODE_GO_WORKSPACE_ID_KEY)?
                .unwrap_or_default()
                .trim()
                .to_string(),
            auth_cookie: self
                .read_setting(OPENCODE_GO_AUTH_COOKIE_KEY)?
                .unwrap_or_default()
                .trim()
                .to_string(),
        })
    }

    pub fn set_opencode_go_connection_params(
        &self,
        params: &OpenCodeGoConnectionParams,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![OPENCODE_GO_WORKSPACE_ID_KEY, params.workspace_id.trim()],
        )?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![OPENCODE_GO_AUTH_COOKIE_KEY, params.auth_cookie.trim()],
        )?;
        Ok(())
    }

    pub fn get_provider_connection_params(
        &self,
        provider_id: &str,
    ) -> AppResult<ProviderConnectionParams> {
        Ok(ProviderConnectionParams {
            api_key: self
                .read_setting(provider_api_key_setting_key(provider_id)?)?
                .unwrap_or_default()
                .trim()
                .to_string(),
        })
    }

    pub fn set_provider_connection_params(
        &self,
        provider_id: &str,
        params: &ProviderConnectionParams,
    ) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                provider_api_key_setting_key(provider_id)?,
                params.api_key.trim()
            ],
        )?;
        Ok(())
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

fn provider_api_key_setting_key(provider_id: &str) -> AppResult<&'static str> {
    match provider_id {
        "minimax-token" => Ok(MINIMAX_TOKEN_PLAN_CN_API_KEY_KEY),
        "deepseek" => Ok(DEEPSEEK_API_KEY_KEY),
        "openrouter" => Ok(OPENROUTER_API_KEY_KEY),
        _ => Err(AppError::Validation(format!(
            "unsupported provider connection params: {provider_id}"
        ))),
    }
}
