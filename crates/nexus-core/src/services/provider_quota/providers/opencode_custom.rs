use std::{collections::BTreeMap, env, fs, path::PathBuf};

use serde::Deserialize;

use crate::{
    error::{AppError, AppResult},
    services::outbound_request_log::OutboundRequestLogger,
};

use super::super::{
    shared::{
        http_client, llm_gateway_quota_from_headers, provider_quota_log_context,
        provider_quota_request_error,
    },
    OpenCodeCustomProvider, ProviderQuotaPollError, ProviderQuotaSnapshot, ProviderQuotaStatus,
    ProviderUsageTransport,
};

const OPENCODE_CONFIG_FILE_ENV: &str = "OPENCODE_CONFIG_FILE";
const OPENCODE_CONFIG_DIR_ENV: &str = "OPENCODE_CONFIG_DIR";
const OPENAI_COMPATIBLE_NPM: &str = "@ai-sdk/openai-compatible";
const OPENAI_NPM: &str = "@ai-sdk/openai";
const PLAN: &str = "OpenCode custom";

#[derive(Debug, Deserialize)]
struct OpenCodeConfigFile {
    #[serde(default)]
    provider: BTreeMap<String, OpenCodeProviderDefinition>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeProviderDefinition {
    name: Option<String>,
    npm: Option<String>,
    options: Option<OpenCodeProviderOptions>,
    #[serde(default)]
    models: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeProviderOptions {
    #[serde(rename = "baseURL")]
    base_url: Option<String>,
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenCodeCustomProviderCredentials {
    pub(crate) provider: OpenCodeCustomProvider,
    pub(crate) api_key: String,
    source: String,
}

pub(crate) async fn quota(
    credentials: OpenCodeCustomProviderCredentials,
    usage_transport: &dyn ProviderUsageTransport,
) -> ProviderQuotaSnapshot {
    if credentials.api_key.is_empty() {
        return super::configured::status(
            &credentials.provider.id,
            ProviderQuotaStatus::NoCreds,
            PLAN,
            &credentials.source,
            None,
        );
    }

    match usage_transport
        .opencode_custom_provider_usage(&credentials)
        .await
    {
        Ok(headers) => {
            let borrowed_headers = headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect::<Vec<_>>();
            let Some(mut snapshot) =
                llm_gateway_quota_from_headers(&credentials.provider.id, PLAN, &borrowed_headers)
            else {
                return super::configured::status(
                    &credentials.provider.id,
                    ProviderQuotaStatus::Failed,
                    PLAN,
                    &credentials.source,
                    Some("response did not contain token quota headers".to_string()),
                );
            };
            snapshot.credential = Some(credentials.source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => super::configured::status(
            &credentials.provider.id,
            ProviderQuotaStatus::Expired,
            PLAN,
            &credentials.source,
            Some("OpenCode custom provider API key was rejected".to_string()),
        ),
        Err(error) => super::configured::status(
            &credentials.provider.id,
            ProviderQuotaStatus::Failed,
            PLAN,
            &credentials.source,
            Some(error.to_string()),
        ),
    }
}

pub fn parse_opencode_custom_providers(content: &str) -> AppResult<Vec<OpenCodeCustomProvider>> {
    Ok(parse_credentials(content)?
        .into_iter()
        .map(|credentials| credentials.provider)
        .collect())
}

fn parse_credentials(content: &str) -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
    let config = serde_json::from_str::<OpenCodeConfigFile>(content)
        .map_err(|error| AppError::Validation(format!("invalid OpenCode config: {error}")))?;
    let mut providers = Vec::new();

    for (id, definition) in config.provider {
        let Some(options) = definition.options else {
            continue;
        };
        let Some(base_url) = options
            .base_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(npm) = definition
            .npm
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(model_id) = definition
            .models
            .keys()
            .find(|value| !value.trim().is_empty())
            .cloned()
        else {
            continue;
        };
        let name = definition
            .name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| id.clone());
        let api_key = options
            .api_key
            .map(|value| value.trim().to_string())
            .unwrap_or_default();

        providers.push(OpenCodeCustomProviderCredentials {
            provider: OpenCodeCustomProvider {
                id: id.clone(),
                name,
                npm,
                base_url,
                model_id,
            },
            api_key,
            source: format!("opencode.json · {id}"),
        });
    }

    Ok(providers)
}

pub(crate) fn read_credentials() -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
    let Some(path) = opencode_config_file_path() else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    parse_credentials(&fs::read_to_string(path)?)
}

fn opencode_config_file_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os(OPENCODE_CONFIG_FILE_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if let Some(dir) = env::var_os(OPENCODE_CONFIG_DIR_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir).join("opencode.json"));
    }
    Some(
        crate::services::paths::home_dir()?
            .join(".config")
            .join("opencode")
            .join("opencode.json"),
    )
}

pub(crate) fn read_opencode_copilot_token() -> Option<String> {
    let content = read_opencode_auth_content()?;
    parse_opencode_copilot_token(&content)
}

pub fn parse_opencode_copilot_token(content: &str) -> Option<String> {
    parse_opencode_auth_token(content, "github-copilot", &["access", "key"])
}

pub fn parse_opencode_provider_token(content: &str, provider_key: &str) -> Option<String> {
    parse_opencode_auth_token(content, provider_key, &["key", "access"])
}

pub(crate) fn read_opencode_auth_token(provider_key: &str) -> Option<String> {
    let content = read_opencode_auth_content()?;
    parse_opencode_provider_token(&content, provider_key)
}

fn read_opencode_auth_content() -> Option<String> {
    let path = match env::var_os("OPENCODE_AUTH_FILE") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => crate::services::paths::home_dir()?
            .join(".local")
            .join("share")
            .join("opencode")
            .join("auth.json"),
    };
    fs::read_to_string(path).ok()
}

fn parse_opencode_auth_token(content: &str, provider_key: &str, fields: &[&str]) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let entry = json.get(provider_key)?;
    for field in fields {
        if let Some(token) = entry.get(*field).and_then(|value| value.as_str()) {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub(crate) async fn fetch_usage(
    credentials: &OpenCodeCustomProviderCredentials,
    request_logger: &OutboundRequestLogger,
) -> Result<Vec<(String, String)>, ProviderQuotaPollError> {
    let (endpoint, body) = match credentials.provider.npm.as_str() {
        OPENAI_COMPATIBLE_NPM => (
            format!(
                "{}/chat/completions",
                credentials.provider.base_url.trim_end_matches('/')
            ),
            serde_json::json!({
                "model": credentials.provider.model_id,
                "messages": [{"role": "user", "content": "Reply with OK."}],
                "stream": false,
                "max_tokens": 1
            })
            .to_string(),
        ),
        OPENAI_NPM => (
            format!(
                "{}/responses",
                credentials.provider.base_url.trim_end_matches('/')
            ),
            serde_json::json!({
                "model": credentials.provider.model_id,
                "input": "Reply with OK."
            })
            .to_string(),
        ),
        npm => {
            return Err(ProviderQuotaPollError::Request(format!(
                "unsupported OpenCode provider package {npm}"
            )));
        }
    };

    let response = request_logger
        .send(
            http_client()
                .post(&endpoint)
                .bearer_auth(&credentials.api_key)
                .header("Content-Type", "application/json")
                .body(body),
            provider_quota_log_context(
                "opencode_custom_provider_usage",
                &credentials.provider.id,
                "POST",
                &endpoint,
            ),
        )
        .await
        .map_err(provider_quota_request_error)?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "OpenCode custom provider endpoint returned {status}"
        )));
    }

    Ok(response
        .headers()
        .iter()
        .filter(|(name, _)| name.as_str().starts_with("x-token-count-"))
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect())
}
