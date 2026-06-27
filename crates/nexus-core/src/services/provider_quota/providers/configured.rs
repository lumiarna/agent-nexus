use serde::Deserialize;

use crate::{
    error::AppResult,
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

use super::{
    super::{
        shared::{
            http_client, percent_to_u8, provider_quota_log_context, provider_quota_request_error,
            shortest_percent_window_used, unix_millis_to_iso,
        },
        ProviderCredentialSource, ProviderQuotaAdapter, ProviderQuotaFuture,
        ProviderQuotaPollError, ProviderQuotaSnapshot, ProviderQuotaStatus, ProviderQuotaWindow,
        ProviderQuotaWindowKind, ProviderUsageTransport,
    },
    opencode_custom,
};

const MINIMAX_TOKEN_PLAN_CN_PROVIDER_ID: &str = "minimax-token";
const MINIMAX_TOKEN_PLAN_CN_OPENCODE_KEY: &str = "minimax-cn-coding-plan";
const MINIMAX_TOKEN_PLAN_CN_USAGE_URL: &str =
    "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains";

const DEEPSEEK_PROVIDER_ID: &str = "deepseek";
const DEEPSEEK_OPENCODE_KEY: &str = "deepseek";
const DEEPSEEK_BALANCE_URL: &str = "https://d3bbv8sr76az5s.cloudfront.net/user/balance";

const OPENROUTER_PROVIDER_ID: &str = "openrouter";
const OPENROUTER_OPENCODE_KEY: &str = "openrouter";
const OPENROUTER_CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Clone, Debug, Deserialize)]
pub struct MiniMaxTokenPlanCnUsageResponse {
    #[serde(default)]
    pub model_remains: Vec<MiniMaxTokenPlanCnModelRemain>,
    pub base_resp: Option<MiniMaxTokenPlanCnBaseResp>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MiniMaxTokenPlanCnBaseResp {
    pub status_code: i64,
    #[serde(default)]
    pub status_msg: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MiniMaxTokenPlanCnModelRemain {
    pub model_name: String,
    pub end_time: Option<i64>,
    pub weekly_end_time: Option<i64>,
    pub current_interval_remaining_percent: Option<f64>,
    pub current_weekly_remaining_percent: Option<f64>,
    pub current_weekly_status: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeepSeekBalanceResponse {
    pub is_available: bool,
    #[serde(default)]
    pub balance_infos: Vec<DeepSeekBalanceInfo>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeepSeekBalanceInfo {
    pub currency: String,
    pub total_balance: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpenRouterCreditsResponse {
    pub data: OpenRouterCreditsData,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpenRouterCreditsData {
    pub total_credits: f64,
    pub total_usage: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct ConfiguredProviderCredentials {
    pub(crate) api_key: String,
    source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfiguredProviderQuotaKind {
    MiniMaxTokenPlanCn,
    DeepSeekBalance,
    OpenRouterCredits,
}

#[derive(Clone, Copy)]
pub(crate) struct ConfiguredProviderQuotaConfig {
    pub(crate) provider_id: &'static str,
    opencode_key: &'static str,
    plan: &'static str,
    manual_credential: &'static str,
    opencode_credential: &'static str,
    auth_error: &'static str,
    kind: ConfiguredProviderQuotaKind,
}

pub(crate) enum ConfiguredProviderUsageResponse {
    MiniMaxTokenPlanCn(MiniMaxTokenPlanCnUsageResponse),
    DeepSeekBalance(DeepSeekBalanceResponse),
    OpenRouterCredits(OpenRouterCreditsResponse),
}

const CONFIGS: &[ConfiguredProviderQuotaConfig] = &[
    ConfiguredProviderQuotaConfig {
        provider_id: MINIMAX_TOKEN_PLAN_CN_PROVIDER_ID,
        opencode_key: MINIMAX_TOKEN_PLAN_CN_OPENCODE_KEY,
        plan: "Token plan",
        manual_credential: "manual API key",
        opencode_credential: "opencode auth.json · minimax-cn-coding-plan",
        auth_error: "MiniMax Token Plan CN API key was rejected",
        kind: ConfiguredProviderQuotaKind::MiniMaxTokenPlanCn,
    },
    ConfiguredProviderQuotaConfig {
        provider_id: DEEPSEEK_PROVIDER_ID,
        opencode_key: DEEPSEEK_OPENCODE_KEY,
        plan: "Balance",
        manual_credential: "manual API key",
        opencode_credential: "opencode auth.json · deepseek",
        auth_error: "DeepSeek API key was rejected",
        kind: ConfiguredProviderQuotaKind::DeepSeekBalance,
    },
    ConfiguredProviderQuotaConfig {
        provider_id: OPENROUTER_PROVIDER_ID,
        opencode_key: OPENROUTER_OPENCODE_KEY,
        plan: "Credits",
        manual_credential: "manual API key",
        opencode_credential: "opencode auth.json · openrouter",
        auth_error: "OpenRouter API key was rejected or lacks credits permission",
        kind: ConfiguredProviderQuotaKind::OpenRouterCredits,
    },
];

pub(crate) struct ConfiguredProviderQuotaAdapter;

impl ProviderQuotaAdapter for ConfiguredProviderQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        "configured-provider"
    }

    fn matches(&self, provider_id: &str) -> bool {
        config(provider_id).is_some()
    }

    fn quota<'a>(
        &'a self,
        provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            let Some(config) = config(provider_id) else {
                return status(
                    provider_id,
                    ProviderQuotaStatus::Failed,
                    "—",
                    "not found",
                    Some("unsupported provider".to_string()),
                );
            };

            let credentials =
                match credential_source.configured_provider_credentials(app_config, config) {
                    Ok(Some(credentials)) => credentials,
                    Ok(None) => {
                        return status(
                            config.provider_id,
                            ProviderQuotaStatus::NoCreds,
                            config.plan,
                            "not found",
                            None,
                        );
                    }
                    Err(error) => {
                        return status(
                            config.provider_id,
                            ProviderQuotaStatus::Failed,
                            config.plan,
                            "not found",
                            Some(error.to_string()),
                        );
                    }
                };

            let usage = usage_transport
                .configured_provider_usage(config, &credentials.api_key)
                .await;
            derive_snapshot(config, credentials, usage)
        })
    }
}

pub(crate) fn config(provider_id: &str) -> Option<&'static ConfiguredProviderQuotaConfig> {
    CONFIGS
        .iter()
        .find(|config| config.provider_id == provider_id)
}

pub(crate) fn status(
    provider_id: &str,
    status: ProviderQuotaStatus,
    plan: &str,
    credential: &str,
    error: Option<String>,
) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status,
        plan: Some(plan.to_string()),
        primary: None,
        windows: Vec::new(),
        credential: Some(credential.to_string()),
        error,
    }
}

pub fn minimax_token_plan_cn_quota_from_usage_response(
    provider_id: &str,
    response: MiniMaxTokenPlanCnUsageResponse,
) -> Option<ProviderQuotaSnapshot> {
    let model = response
        .model_remains
        .into_iter()
        .find(|model| model.model_name == "general")?;

    let mut windows = Vec::new();
    if let Some(remaining) = model.current_interval_remaining_percent {
        windows.push(ProviderQuotaWindow {
            label: "5-hour limit".to_string(),
            kind: ProviderQuotaWindowKind::Rolling,
            used: percent_to_u8(100.0 - remaining),
            value_label: None,
            value_only: false,
            reset_at: model.end_time.and_then(unix_millis_to_iso),
            unlimited: false,
        });
    }
    if model.current_weekly_status == Some(1) {
        if let Some(remaining) = model.current_weekly_remaining_percent {
            windows.push(ProviderQuotaWindow {
                label: "Weekly limit".to_string(),
                kind: ProviderQuotaWindowKind::Weekly,
                used: percent_to_u8(100.0 - remaining),
                value_label: None,
                value_only: false,
                reset_at: model.weekly_end_time.and_then(unix_millis_to_iso),
                unlimited: false,
            });
        }
    }

    if windows.is_empty() {
        return None;
    }

    let primary = shortest_percent_window_used(&windows);
    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some("Token plan".to_string()),
        primary,
        windows,
        credential: None,
        error: None,
    })
}

pub fn deepseek_balance_quota_from_usage_response(
    provider_id: &str,
    response: DeepSeekBalanceResponse,
) -> Option<ProviderQuotaSnapshot> {
    if response.balance_infos.is_empty() {
        return None;
    }

    let windows = response
        .balance_infos
        .into_iter()
        .map(|info| ProviderQuotaWindow {
            label: format!("{} balance", info.currency),
            kind: ProviderQuotaWindowKind::Monthly,
            used: 0,
            value_label: Some(format!("{} {}", info.total_balance, info.currency)),
            value_only: true,
            reset_at: None,
            unlimited: false,
        })
        .collect();

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some("Balance".to_string()),
        primary: None,
        windows,
        credential: None,
        error: if response.is_available {
            None
        } else {
            Some("Insufficient balance".to_string())
        },
    })
}

pub fn openrouter_credits_quota_from_usage_response(
    provider_id: &str,
    response: OpenRouterCreditsResponse,
) -> Option<ProviderQuotaSnapshot> {
    let total = response.data.total_credits;
    let used = response.data.total_usage;
    if !total.is_finite() || !used.is_finite() {
        return None;
    }

    let remaining = total - used;

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some("Credits".to_string()),
        primary: None,
        windows: vec![
            ProviderQuotaWindow {
                label: "Credit used".to_string(),
                kind: ProviderQuotaWindowKind::Monthly,
                used: 0,
                value_label: Some(format!("{} credits used", format_credit_value(used))),
                value_only: true,
                reset_at: None,
                unlimited: false,
            },
            ProviderQuotaWindow {
                label: "Credit balance".to_string(),
                kind: ProviderQuotaWindowKind::Monthly,
                used: 0,
                value_label: Some(format!(
                    "{} credits balance",
                    format_credit_value(remaining)
                )),
                value_only: true,
                reset_at: None,
                unlimited: false,
            },
        ],
        credential: None,
        error: if remaining > 0.0 {
            None
        } else {
            Some("No credits remaining".to_string())
        },
    })
}

fn derive_snapshot(
    config: &'static ConfiguredProviderQuotaConfig,
    credentials: ConfiguredProviderCredentials,
    usage: Result<ConfiguredProviderUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match usage {
        Ok(response) => {
            let snapshot = match response {
                ConfiguredProviderUsageResponse::MiniMaxTokenPlanCn(response) => {
                    minimax_token_plan_cn_quota_from_usage_response(config.provider_id, response)
                }
                ConfiguredProviderUsageResponse::DeepSeekBalance(response) => {
                    deepseek_balance_quota_from_usage_response(config.provider_id, response)
                }
                ConfiguredProviderUsageResponse::OpenRouterCredits(response) => {
                    openrouter_credits_quota_from_usage_response(config.provider_id, response)
                }
            };

            let Some(mut snapshot) = snapshot else {
                return status(
                    config.provider_id,
                    ProviderQuotaStatus::Failed,
                    config.plan,
                    &credentials.source,
                    Some(format!(
                        "{} response did not contain recognizable quota data",
                        config.provider_id
                    )),
                );
            };
            snapshot.credential = Some(credentials.source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => status(
            config.provider_id,
            ProviderQuotaStatus::Expired,
            config.plan,
            &credentials.source,
            Some(config.auth_error.to_string()),
        ),
        Err(error) => status(
            config.provider_id,
            ProviderQuotaStatus::Failed,
            config.plan,
            &credentials.source,
            Some(error.to_string()),
        ),
    }
}

pub(crate) fn read_credentials(
    app_config: &AppConfigService,
    config: &'static ConfiguredProviderQuotaConfig,
) -> AppResult<Option<ConfiguredProviderCredentials>> {
    let manual = app_config.get_provider_connection_params(config.provider_id)?;
    if !manual.api_key.is_empty() {
        return Ok(Some(ConfiguredProviderCredentials {
            api_key: manual.api_key,
            source: config.manual_credential.to_string(),
        }));
    }

    Ok(
        opencode_custom::read_opencode_auth_token(config.opencode_key).map(|api_key| {
            ConfiguredProviderCredentials {
                api_key,
                source: config.opencode_credential.to_string(),
            }
        }),
    )
}

pub(crate) async fn fetch_usage(
    config: &'static ConfiguredProviderQuotaConfig,
    api_key: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<ConfiguredProviderUsageResponse, ProviderQuotaPollError> {
    match config.kind {
        ConfiguredProviderQuotaKind::MiniMaxTokenPlanCn => {
            fetch_minimax_token_plan_cn_usage(api_key, request_logger)
                .await
                .map(ConfiguredProviderUsageResponse::MiniMaxTokenPlanCn)
        }
        ConfiguredProviderQuotaKind::DeepSeekBalance => {
            fetch_deepseek_balance(api_key, request_logger)
                .await
                .map(ConfiguredProviderUsageResponse::DeepSeekBalance)
        }
        ConfiguredProviderQuotaKind::OpenRouterCredits => {
            fetch_openrouter_credits(api_key, request_logger)
                .await
                .map(ConfiguredProviderUsageResponse::OpenRouterCredits)
        }
    }
}

async fn fetch_minimax_token_plan_cn_usage(
    api_key: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<MiniMaxTokenPlanCnUsageResponse, ProviderQuotaPollError> {
    let response = request_logger
        .send(
            http_client()
                .get(MINIMAX_TOKEN_PLAN_CN_USAGE_URL)
                .bearer_auth(api_key)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json"),
            provider_quota_log_context(
                "minimax_token_plan_cn_usage",
                MINIMAX_TOKEN_PLAN_CN_PROVIDER_ID,
                "GET",
                MINIMAX_TOKEN_PLAN_CN_USAGE_URL,
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
            "MiniMax Token Plan CN usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    let parsed = serde_json::from_str::<MiniMaxTokenPlanCnUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    if let Some(base_resp) = &parsed.base_resp {
        if base_resp.status_code != 0 {
            return Err(ProviderQuotaPollError::AuthRequired);
        }
    }
    Ok(parsed)
}

async fn fetch_deepseek_balance(
    api_key: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<DeepSeekBalanceResponse, ProviderQuotaPollError> {
    let response = request_logger
        .send(
            http_client()
                .get(DEEPSEEK_BALANCE_URL)
                .bearer_auth(api_key)
                .header("Host", "api.deepseek.com")
                .header("Accept", "application/json"),
            provider_quota_log_context(
                "deepseek_balance",
                DEEPSEEK_PROVIDER_ID,
                "GET",
                DEEPSEEK_BALANCE_URL,
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
            "DeepSeek balance endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<DeepSeekBalanceResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

async fn fetch_openrouter_credits(
    api_key: &str,
    request_logger: &OutboundRequestLogger,
) -> Result<OpenRouterCreditsResponse, ProviderQuotaPollError> {
    let response = request_logger
        .send(
            http_client()
                .get(OPENROUTER_CREDITS_URL)
                .bearer_auth(api_key)
                .header("Accept", "application/json"),
            provider_quota_log_context(
                "openrouter_credits",
                OPENROUTER_PROVIDER_ID,
                "GET",
                OPENROUTER_CREDITS_URL,
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
            "OpenRouter credits endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<OpenRouterCreditsResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn format_credit_value(value: f64) -> String {
    format!("{value:.2}")
}
