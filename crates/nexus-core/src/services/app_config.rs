use std::{path::PathBuf, sync::Arc};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::agent_by_name,
    services::paths::{path_to_string, resolve_local_path},
};

pub const CLAUDE_CONFIG_DIR_KEY: &str = "CLAUDE_CONFIG_DIR";
const DEFAULT_CLAUDE_CONFIG_DIR: &str = "~/.claude";

pub const CODEX_CONFIG_DIR_KEY: &str = "CODEX_CONFIG_DIR";
const DEFAULT_CODEX_CONFIG_DIR: &str = "~/.codex";

pub const COPILOT_GITHUB_TOKEN_KEY: &str = "COPILOT_GITHUB_TOKEN";
pub const OPENCODE_GO_WORKSPACE_ID_KEY: &str = "OPENCODE_GO_WORKSPACE_ID";
pub const OPENCODE_GO_AUTH_COOKIE_KEY: &str = "OPENCODE_GO_AUTH_COOKIE";
pub const QODER_SESSION_COOKIE_KEY: &str = "QODER_SESSION_COOKIE";
const PROVIDER_ORDER_KEY: &str = "PROVIDER_ORDER";
const MINIMAX_TOKEN_PLAN_CN_API_KEY_KEY: &str = "PROVIDER_API_KEY_MINIMAX_TOKEN";
const DEEPSEEK_API_KEY_KEY: &str = "PROVIDER_API_KEY_DEEPSEEK";
const OPENROUTER_API_KEY_KEY: &str = "PROVIDER_API_KEY_OPENROUTER";
const PROVIDER_CARD_VISIBILITY_KEY: &str = "PROVIDER_CARD_VISIBILITY";
const DISABLED_AGENTS_KEY: &str = "DISABLED_AGENTS";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayMetric {
    Used,
    Remaining,
}

impl Default for TrayMetric {
    fn default() -> Self {
        Self::Remaining
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoConnectionParams {
    pub workspace_id: String,
    pub auth_cookie: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QoderConnectionParams {
    pub session_cookie: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionParams {
    pub api_key: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDisplayPreferences {
    pub card_visibility: Vec<String>,
    #[serde(default)]
    pub tray_metric: TrayMetric,
    /// Provider ids whose quota is shown as a Windows-taskbar tray icon.
    /// A `Surface Preference` independent of `card_visibility`; only providers
    /// that expose a "shortest window used" (`primary`) can be enabled here,
    /// but that gating lives in the front end since it depends on live quota.
    #[serde(default)]
    pub tray_visibility: Vec<String>,
}

/// User preference for which Agents are disabled. A disabled Agent is dropped
/// from the Skill / Prompt Agent Matrix and the assets it sources are hidden;
/// its `Agent Capability Surface` still exists. Names are canonical Agent names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDisplayPreferences {
    pub disabled: Vec<String>,
    /// Default Global entry Agent used when a Project custom `Skill` (which has
    /// no `Source Agent`) is propagated to Global. `None` falls back to the
    /// canonical-leftmost Agent (`Generic Agent`) in the front end. Must be a
    /// Skill-capable, non-disabled Agent; disabling it clears this back to `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_global_entry_agent: Option<String>,
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

    pub fn get_qoder_connection_params(&self) -> AppResult<QoderConnectionParams> {
        Ok(QoderConnectionParams {
            session_cookie: self
                .read_setting(QODER_SESSION_COOKIE_KEY)?
                .unwrap_or_default()
                .trim()
                .to_string(),
        })
    }

    pub fn set_qoder_connection_params(&self, params: &QoderConnectionParams) -> AppResult<()> {
        let conn = self.db.connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![QODER_SESSION_COOKIE_KEY, params.session_cookie.trim()],
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
    pub fn get_provider_order(&self) -> AppResult<Vec<String>> {
        let raw = self.read_setting(PROVIDER_ORDER_KEY)?.unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }

        let provider_ids = serde_json::from_str::<Vec<String>>(&raw).map_err(|error| {
            AppError::Validation(format!("invalid provider order setting: {error}"))
        })?;
        normalize_provider_order(provider_ids)
    }

    pub fn set_provider_order(&self, provider_ids: &[String]) -> AppResult<Vec<String>> {
        let normalized = normalize_provider_order(provider_ids.to_vec())?;
        self.write_json_setting(PROVIDER_ORDER_KEY, &normalized)?;
        Ok(normalized)
    }

    pub fn get_provider_display_preferences(&self) -> AppResult<ProviderDisplayPreferences> {
        let raw = self
            .read_setting(PROVIDER_CARD_VISIBILITY_KEY)?
            .unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(ProviderDisplayPreferences::default());
        }

        let preferences =
            serde_json::from_str::<ProviderDisplayPreferences>(&raw).map_err(|error| {
                AppError::Validation(format!(
                    "invalid provider display preferences setting: {error}"
                ))
            })?;
        Ok(ProviderDisplayPreferences {
            card_visibility: normalize_provider_order(preferences.card_visibility)?,
            tray_metric: normalize_tray_metric(preferences.tray_metric)?,
            tray_visibility: normalize_provider_order(preferences.tray_visibility)?,
        })
    }

    pub fn set_provider_display_preferences(
        &self,
        preferences: &ProviderDisplayPreferences,
    ) -> AppResult<ProviderDisplayPreferences> {
        let normalized = ProviderDisplayPreferences {
            card_visibility: normalize_provider_order(preferences.card_visibility.clone())?,
            tray_metric: normalize_tray_metric(preferences.tray_metric)?,
            tray_visibility: normalize_provider_order(preferences.tray_visibility.clone())?,
        };
        self.write_json_setting(PROVIDER_CARD_VISIBILITY_KEY, &normalized)?;
        Ok(normalized)
    }

    pub fn get_agent_display_preferences(&self) -> AppResult<AgentDisplayPreferences> {
        let raw = self.read_setting(DISABLED_AGENTS_KEY)?.unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(AgentDisplayPreferences::default());
        }

        let preferences =
            serde_json::from_str::<AgentDisplayPreferences>(&raw).map_err(|error| {
                AppError::Validation(format!(
                    "invalid agent display preferences setting: {error}"
                ))
            })?;
        let disabled = normalize_agent_names(preferences.disabled)?;
        Ok(AgentDisplayPreferences {
            default_global_entry_agent: normalize_default_global_entry_agent(
                preferences.default_global_entry_agent,
                &disabled,
            )?,
            disabled,
        })
    }

    pub fn set_agent_display_preferences(
        &self,
        preferences: &AgentDisplayPreferences,
    ) -> AppResult<AgentDisplayPreferences> {
        let disabled = normalize_agent_names(preferences.disabled.clone())?;
        let normalized = AgentDisplayPreferences {
            default_global_entry_agent: normalize_default_global_entry_agent(
                preferences.default_global_entry_agent.clone(),
                &disabled,
            )?,
            disabled,
        };
        self.write_json_setting(DISABLED_AGENTS_KEY, &normalized)?;
        Ok(normalized)
    }

    fn write_json_setting<T: Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        let conn = self.db.connection()?;
        let value = serde_json::to_string(value)
            .map_err(|error| AppError::Internal(format!("serialize {key}: {error}")))?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
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

fn normalize_tray_metric(metric: TrayMetric) -> AppResult<TrayMetric> {
    match metric {
        TrayMetric::Used | TrayMetric::Remaining => Ok(metric),
    }
}

fn normalize_provider_order(provider_ids: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(provider_ids.len());

    for provider_id in provider_ids {
        let provider_id = provider_id.trim();
        if provider_id.is_empty() {
            return Err(AppError::Validation(
                "provider order cannot contain empty ids".to_string(),
            ));
        }
        if normalized.iter().any(|existing| existing == provider_id) {
            return Err(AppError::Validation(format!(
                "provider order contains duplicate id: {provider_id}"
            )));
        }
        normalized.push(provider_id.to_string());
    }

    Ok(normalized)
}

fn normalize_agent_names(names: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(names.len());

    for name in names {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation(
                "disabled agents cannot contain empty names".to_string(),
            ));
        }
        if agent_by_name(name).is_none() {
            return Err(AppError::Validation(format!("unknown agent: {name}")));
        }
        if normalized.iter().any(|existing| existing == name) {
            return Err(AppError::Validation(format!(
                "disabled agents contains duplicate: {name}"
            )));
        }
        normalized.push(name.to_string());
    }

    Ok(normalized)
}

/// Validate the Default Global entry Agent against the same invariants the UI
/// enforces: it must be a known, Skill-capable Agent. If it is currently
/// disabled the preference is cleared to `None` rather than pointing at a
/// hidden Agent.
fn normalize_default_global_entry_agent(
    agent: Option<String>,
    disabled: &[String],
) -> AppResult<Option<String>> {
    let Some(name) = agent else {
        return Ok(None);
    };
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let surface = agent_by_name(name)
        .ok_or_else(|| AppError::Validation(format!("unknown agent: {name}")))?;
    if surface.skill.is_none() {
        return Err(AppError::Validation(format!(
            "default global entry agent must be Skill-capable: {name}"
        )));
    }
    if disabled.iter().any(|d| d == name) {
        return Ok(None);
    }
    Ok(Some(name.to_string()))
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
