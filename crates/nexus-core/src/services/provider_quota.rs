use std::{
    env, fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use std::process::Command;

use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    error::{AppError, AppResult},
    services::app_config::{AppConfigService, OpenCodeGoConnectionParams},
};

const CLAUDE_CODE_PROVIDER_ID: &str = "claude";
const CLAUDE_CODE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

const CODEX_PROVIDER_ID: &str = "codex";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

const COPILOT_PROVIDER_ID: &str = "copilot";
const COPILOT_USAGE_URL: &str = "https://api.github.com/copilot_internal/user";

const OPENCODE_GO_PROVIDER_ID: &str = "opencode-go";
const OPENCODE_GO_BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const MINIMAX_TOKEN_PLAN_CN_PROVIDER_ID: &str = "minimax-token";
const MINIMAX_TOKEN_PLAN_CN_OPENCODE_KEY: &str = "minimax-cn-coding-plan";
const MINIMAX_TOKEN_PLAN_CN_USAGE_URL: &str =
    "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains";

const DEEPSEEK_PROVIDER_ID: &str = "deepseek";
const DEEPSEEK_OPENCODE_KEY: &str = "deepseek";
const DEEPSEEK_BALANCE_URL: &str = "https://d3bbv8sr76az5s.cloudfront.net/user/balance";
// 为什么不直接用 api.deepseek.com？
// 部分企业 DNS 会将该域名解析到被阻断的腾讯 EdgeOne CDN IP（58.49.197.113、
// 183.131.191.171），导致 TLS 443 端口不通。公网 DNS（8.8.8.8）解析到 AWS
// CloudFront（d3bbv8sr76az5s.cloudfront.net），该域名直连和代理均可达。通过
// CloudFront 域名 + Host: api.deepseek.com 请求头可绕过 DNS 污染。
// 详见 docs/adr/0002-deepseek-cloudfront-endpoint.md

const OPENROUTER_PROVIDER_ID: &str = "openrouter";
const OPENROUTER_OPENCODE_KEY: &str = "openrouter";
const OPENROUTER_CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderQuotaStatus {
    Available,
    Expired,
    Failed,
    #[serde(rename = "nocreds")]
    NoCreds,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderQuotaWindowKind {
    Rolling,
    Weekly,
    Monthly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuotaWindow {
    pub label: String,
    pub kind: ProviderQuotaWindowKind,
    pub used: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_label: Option<String>,
    pub value_only: bool,
    pub reset_at: Option<String>,
    pub unlimited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuotaSnapshot {
    pub provider_id: String,
    pub status: ProviderQuotaStatus,
    pub plan: Option<String>,
    pub primary: Option<u8>,
    pub windows: Vec<ProviderQuotaWindow>,
    pub credential: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClaudeCodeUsageResponse {
    pub five_hour: Option<ClaudeCodeUsageBucket>,
    pub seven_day: Option<ClaudeCodeUsageBucket>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClaudeCodeUsageBucket {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexUsageResponse {
    pub plan_type: Option<String>,
    pub rate_limit: Option<CodexRateLimit>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexRateLimit {
    pub primary_window: Option<CodexRateLimitWindow>,
    pub secondary_window: Option<CodexRateLimitWindow>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodexRateLimitWindow {
    pub used_percent: Option<f64>,
    pub limit_window_seconds: Option<i64>,
    pub reset_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotUsageResponse {
    pub copilot_plan: Option<String>,
    pub quota_reset_date: Option<String>,
    pub quota_snapshots: Option<CopilotQuotaSnapshots>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotQuotaSnapshots {
    pub premium_interactions: Option<CopilotQuotaDetail>,
    pub chat: Option<CopilotQuotaDetail>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CopilotQuotaDetail {
    pub entitlement: Option<i64>,
    pub remaining: Option<f64>,
    pub percent_remaining: Option<f64>,
    pub unlimited: Option<bool>,
}

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
struct ClaudeCodeCredentials {
    access_token: String,
    expires_at: Option<i64>,
    plan: Option<String>,
    source: String,
}

#[derive(Clone, Debug)]
struct OpenCodeGoCredentials {
    workspace_id: String,
    auth_cookie: String,
    source: String,
}

#[derive(Clone, Debug)]
struct ConfiguredProviderCredentials {
    api_key: String,
    source: String,
}

type ClaudeCodeUsageFuture<'a> = Pin<
    Box<dyn Future<Output = Result<ClaudeCodeUsageResponse, ProviderQuotaPollError>> + Send + 'a>,
>;
type CodexUsageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CodexUsageResponse, ProviderQuotaPollError>> + Send + 'a>>;
type CopilotUsageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CopilotUsageResponse, ProviderQuotaPollError>> + Send + 'a>>;
type OpenCodeGoPageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, ProviderQuotaPollError>> + Send + 'a>>;
type ConfiguredProviderUsageFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<ConfiguredProviderUsageResponse, ProviderQuotaPollError>>
            + Send
            + 'a,
    >,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfiguredProviderQuotaKind {
    MiniMaxTokenPlanCn,
    DeepSeekBalance,
    OpenRouterCredits,
}

#[derive(Clone, Copy)]
struct ConfiguredProviderQuotaConfig {
    provider_id: &'static str,
    opencode_key: &'static str,
    plan: &'static str,
    manual_credential: &'static str,
    opencode_credential: &'static str,
    auth_error: &'static str,
    kind: ConfiguredProviderQuotaKind,
}

enum ConfiguredProviderUsageResponse {
    MiniMaxTokenPlanCn(MiniMaxTokenPlanCnUsageResponse),
    DeepSeekBalance(DeepSeekBalanceResponse),
    OpenRouterCredits(OpenRouterCreditsResponse),
}

const CONFIGURED_PROVIDER_QUOTA_CONFIGS: &[ConfiguredProviderQuotaConfig] = &[
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

trait ProviderCredentialSource: Send + Sync {
    fn claude_code_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<ClaudeCodeCredentials>>;
    fn codex_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<CodexCredentials>>;
    fn copilot_token(&self, app_config: &AppConfigService) -> AppResult<Option<String>>;
    fn opencode_go_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<OpenCodeGoCredentials>>;
    fn configured_provider_credentials(
        &self,
        app_config: &AppConfigService,
        config: &'static ConfiguredProviderQuotaConfig,
    ) -> AppResult<Option<ConfiguredProviderCredentials>>;
}

trait ProviderUsageTransport: Send + Sync {
    fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a>;
    fn codex_usage<'a>(
        &'a self,
        access_token: &'a str,
        account_id: Option<&'a str>,
    ) -> CodexUsageFuture<'a>;
    fn copilot_usage<'a>(&'a self, token: &'a str) -> CopilotUsageFuture<'a>;
    fn opencode_go_page<'a>(
        &'a self,
        workspace_id: &'a str,
        auth_cookie: &'a str,
    ) -> OpenCodeGoPageFuture<'a>;
    fn configured_provider_usage<'a>(
        &'a self,
        config: &'static ConfiguredProviderQuotaConfig,
        api_key: &'a str,
    ) -> ConfiguredProviderUsageFuture<'a>;
}

#[derive(Clone)]
struct LocalCredentialSource;

#[derive(Clone)]
struct HttpUsageTransport;

type ProviderQuotaFuture<'a> = Pin<Box<dyn Future<Output = ProviderQuotaSnapshot> + Send + 'a>>;

trait ProviderQuotaAdapter: Sync {
    fn provider_id(&self) -> &'static str;

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    fn quota<'a>(
        &'a self,
        provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a>;

    fn matches(&self, provider_id: &str) -> bool {
        self.provider_id() == provider_id || self.aliases().contains(&provider_id)
    }
}

static CLAUDE_CODE_QUOTA_ADAPTER: ClaudeCodeQuotaAdapter = ClaudeCodeQuotaAdapter;
static CODEX_QUOTA_ADAPTER: CodexQuotaAdapter = CodexQuotaAdapter;
static COPILOT_QUOTA_ADAPTER: CopilotQuotaAdapter = CopilotQuotaAdapter;
static OPENCODE_GO_QUOTA_ADAPTER: OpenCodeGoQuotaAdapter = OpenCodeGoQuotaAdapter;
static CONFIGURED_PROVIDER_QUOTA_ADAPTER: ConfiguredProviderQuotaAdapter =
    ConfiguredProviderQuotaAdapter;

#[derive(Clone)]
pub struct ProviderQuotaService {
    app_config: AppConfigService,
    credential_source: Arc<dyn ProviderCredentialSource>,
    usage_transport: Arc<dyn ProviderUsageTransport>,
}

impl ProviderQuotaService {
    pub fn new(app_config: AppConfigService) -> Self {
        Self {
            app_config,
            credential_source: Arc::new(LocalCredentialSource),
            usage_transport: Arc::new(HttpUsageTransport),
        }
    }

    pub async fn get_provider_quota(&self, provider_id: &str) -> AppResult<ProviderQuotaSnapshot> {
        for adapter in provider_quota_adapters() {
            if adapter.matches(provider_id) {
                return Ok(adapter
                    .quota(
                        provider_id,
                        &self.app_config,
                        self.credential_source.as_ref(),
                        self.usage_transport.as_ref(),
                    )
                    .await);
            }
        }
        Err(AppError::Validation("unsupported provider".to_string()))
    }
}

fn provider_quota_adapters() -> [&'static dyn ProviderQuotaAdapter; 5] {
    [
        &CLAUDE_CODE_QUOTA_ADAPTER,
        &CODEX_QUOTA_ADAPTER,
        &COPILOT_QUOTA_ADAPTER,
        &OPENCODE_GO_QUOTA_ADAPTER,
        &CONFIGURED_PROVIDER_QUOTA_ADAPTER,
    ]
}

impl ProviderCredentialSource for LocalCredentialSource {
    fn claude_code_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<ClaudeCodeCredentials>> {
        #[cfg(target_os = "macos")]
        if let Some(credentials) = read_claude_code_credentials_from_keychain()? {
            return Ok(Some(credentials));
        }

        let path = app_config
            .get_claude_config_dir()?
            .join(".credentials.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        parse_claude_code_credentials(&content, path)
    }

    fn codex_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<CodexCredentials>> {
        let path = app_config.get_codex_config_dir()?.join("auth.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        parse_codex_credentials(&content, &path)
    }

    fn copilot_token(&self, app_config: &AppConfigService) -> AppResult<Option<String>> {
        if let Some(token) = app_config
            .get_copilot_github_token()?
            .filter(|token| !token.is_empty())
        {
            return Ok(Some(token));
        }
        Ok(read_opencode_copilot_token())
    }

    fn opencode_go_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<OpenCodeGoCredentials>> {
        let OpenCodeGoConnectionParams {
            workspace_id,
            auth_cookie,
        } = app_config.get_opencode_go_connection_params()?;
        if workspace_id.is_empty() || auth_cookie.is_empty() {
            return Ok(None);
        }
        Ok(Some(OpenCodeGoCredentials {
            workspace_id,
            auth_cookie,
            source: "manual workspace id + auth cookie".to_string(),
        }))
    }

    fn configured_provider_credentials(
        &self,
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
            read_opencode_auth_token(config.opencode_key).map(|api_key| {
                ConfiguredProviderCredentials {
                    api_key,
                    source: config.opencode_credential.to_string(),
                }
            }),
        )
    }
}

impl ProviderUsageTransport for HttpUsageTransport {
    fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a> {
        Box::pin(fetch_claude_code_usage(access_token))
    }

    fn codex_usage<'a>(
        &'a self,
        access_token: &'a str,
        account_id: Option<&'a str>,
    ) -> CodexUsageFuture<'a> {
        Box::pin(fetch_codex_usage(access_token, account_id))
    }

    fn copilot_usage<'a>(&'a self, token: &'a str) -> CopilotUsageFuture<'a> {
        Box::pin(fetch_copilot_usage(token))
    }

    fn opencode_go_page<'a>(
        &'a self,
        workspace_id: &'a str,
        auth_cookie: &'a str,
    ) -> OpenCodeGoPageFuture<'a> {
        Box::pin(fetch_opencode_go_page(workspace_id, auth_cookie))
    }

    fn configured_provider_usage<'a>(
        &'a self,
        config: &'static ConfiguredProviderQuotaConfig,
        api_key: &'a str,
    ) -> ConfiguredProviderUsageFuture<'a> {
        Box::pin(fetch_configured_provider_usage(config, api_key))
    }
}

struct ClaudeCodeQuotaAdapter;

impl ProviderQuotaAdapter for ClaudeCodeQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        CLAUDE_CODE_PROVIDER_ID
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["claude-code"]
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_snapshot(app_config, credential_source, usage_transport)
                .await
        })
    }
}

impl ClaudeCodeQuotaAdapter {
    async fn quota_snapshot(
        &self,
        app_config: &AppConfigService,
        credential_source: &dyn ProviderCredentialSource,
        usage_transport: &dyn ProviderUsageTransport,
    ) -> ProviderQuotaSnapshot {
        let credentials = match credential_source.claude_code_credentials(app_config) {
            Ok(Some(credentials)) => credentials,
            Ok(None) => return claude_code_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return claude_code_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        if is_token_expired(credentials.expires_at) {
            return ProviderQuotaSnapshot {
                provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Expired,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source),
                error: Some("Claude Code token expired; run claude /login to refresh".to_string()),
            };
        }

        let usage = usage_transport
            .claude_code_usage(&credentials.access_token)
            .await;
        derive_claude_code_snapshot(credentials, usage)
    }
}

struct CodexQuotaAdapter;

impl ProviderQuotaAdapter for CodexQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        CODEX_PROVIDER_ID
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_snapshot(app_config, credential_source, usage_transport)
                .await
        })
    }
}

impl CodexQuotaAdapter {
    async fn quota_snapshot(
        &self,
        app_config: &AppConfigService,
        credential_source: &dyn ProviderCredentialSource,
        usage_transport: &dyn ProviderUsageTransport,
    ) -> ProviderQuotaSnapshot {
        let credentials = match credential_source.codex_credentials(app_config) {
            Ok(Some(credentials)) => credentials,
            Ok(None) => return codex_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return codex_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        let usage = usage_transport
            .codex_usage(&credentials.access_token, credentials.account_id.as_deref())
            .await;
        derive_codex_snapshot(credentials, usage)
    }
}

struct CopilotQuotaAdapter;

impl ProviderQuotaAdapter for CopilotQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        COPILOT_PROVIDER_ID
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_snapshot(app_config, credential_source, usage_transport)
                .await
        })
    }
}

impl CopilotQuotaAdapter {
    async fn quota_snapshot(
        &self,
        app_config: &AppConfigService,
        credential_source: &dyn ProviderCredentialSource,
        usage_transport: &dyn ProviderUsageTransport,
    ) -> ProviderQuotaSnapshot {
        let token = match credential_source.copilot_token(app_config) {
            Ok(Some(token)) => token,
            Ok(None) => return copilot_status(ProviderQuotaStatus::NoCreds, "not found"),
            Err(error) => {
                return copilot_status(ProviderQuotaStatus::Failed, error.to_string().as_str());
            }
        };

        let usage = usage_transport.copilot_usage(&token).await;
        derive_copilot_snapshot(usage)
    }
}

struct OpenCodeGoQuotaAdapter;

impl ProviderQuotaAdapter for OpenCodeGoQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        OPENCODE_GO_PROVIDER_ID
    }

    fn quota<'a>(
        &'a self,
        _provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_snapshot(app_config, credential_source, usage_transport)
                .await
        })
    }
}

impl OpenCodeGoQuotaAdapter {
    async fn quota_snapshot(
        &self,
        app_config: &AppConfigService,
        credential_source: &dyn ProviderCredentialSource,
        usage_transport: &dyn ProviderUsageTransport,
    ) -> ProviderQuotaSnapshot {
        let credentials = match credential_source.opencode_go_credentials(app_config) {
            Ok(Some(credentials)) => credentials,
            Ok(None) => {
                return opencode_go_status(
                    ProviderQuotaStatus::NoCreds,
                    "manual workspace id + auth cookie",
                    None,
                );
            }
            Err(error) => {
                return opencode_go_status(
                    ProviderQuotaStatus::Failed,
                    "manual workspace id + auth cookie",
                    Some(error.to_string()),
                );
            }
        };

        let html = usage_transport
            .opencode_go_page(&credentials.workspace_id, &credentials.auth_cookie)
            .await;
        derive_opencode_go_snapshot(credentials, html)
    }
}

struct ConfiguredProviderQuotaAdapter;

impl ProviderQuotaAdapter for ConfiguredProviderQuotaAdapter {
    fn provider_id(&self) -> &'static str {
        "configured-provider"
    }

    fn matches(&self, provider_id: &str) -> bool {
        configured_provider_quota_config(provider_id).is_some()
    }

    fn quota<'a>(
        &'a self,
        provider_id: &'a str,
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> ProviderQuotaFuture<'a> {
        Box::pin(async move {
            let Some(config) = configured_provider_quota_config(provider_id) else {
                return configured_provider_status(
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
                        return configured_provider_status(
                            config.provider_id,
                            ProviderQuotaStatus::NoCreds,
                            config.plan,
                            "not found",
                            None,
                        );
                    }
                    Err(error) => {
                        return configured_provider_status(
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
            derive_configured_provider_snapshot(config, credentials, usage)
        })
    }
}

fn configured_provider_quota_config(
    provider_id: &str,
) -> Option<&'static ConfiguredProviderQuotaConfig> {
    CONFIGURED_PROVIDER_QUOTA_CONFIGS
        .iter()
        .find(|config| config.provider_id == provider_id)
}

fn configured_provider_status(
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

pub fn claude_code_quota_from_usage_response(
    provider_id: &str,
    plan: Option<String>,
    response: ClaudeCodeUsageResponse,
) -> ProviderQuotaSnapshot {
    let windows = [
        response
            .five_hour
            .map(|bucket| quota_window("5-hour limit", ProviderQuotaWindowKind::Rolling, bucket)),
        response
            .seven_day
            .map(|bucket| quota_window("Weekly limit", ProviderQuotaWindowKind::Weekly, bucket)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let primary = shortest_percent_window_used(&windows);

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

pub fn codex_quota_from_usage_response(
    provider_id: &str,
    plan: Option<String>,
    response: CodexUsageResponse,
) -> ProviderQuotaSnapshot {
    let mut windows = Vec::new();
    if let Some(rate_limit) = response.rate_limit {
        for window in [rate_limit.primary_window, rate_limit.secondary_window]
            .into_iter()
            .flatten()
        {
            if let Some(used_percent) = window.used_percent {
                let (kind, label) = codex_window_meta(window.limit_window_seconds);
                windows.push(ProviderQuotaWindow {
                    label,
                    kind,
                    used: percent_to_u8(used_percent),
                    value_label: None,
                    value_only: false,
                    reset_at: window.reset_at.and_then(unix_seconds_to_iso),
                    unlimited: false,
                });
            }
        }
    }

    let primary = shortest_percent_window_used(&windows);

    let plan = response
        .plan_type
        .map(|raw| codex_plan_label(&raw))
        .or(plan);

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

pub fn copilot_quota_from_usage_response(
    provider_id: &str,
    response: CopilotUsageResponse,
) -> ProviderQuotaSnapshot {
    let reset_at = response
        .quota_reset_date
        .as_deref()
        .and_then(copilot_reset_to_iso);
    let snapshots = response.quota_snapshots;

    let windows = [
        snapshots
            .as_ref()
            .and_then(|s| s.premium_interactions.as_ref())
            .and_then(|detail| copilot_window("Premium Interactions", detail, reset_at.clone())),
        snapshots
            .as_ref()
            .and_then(|s| s.chat.as_ref())
            .and_then(|detail| copilot_window("Chat Quota", detail, reset_at.clone())),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let primary = shortest_percent_window_used(&windows);
    let plan = response.copilot_plan.as_deref().map(copilot_plan_label);

    ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    }
}

pub fn opencode_go_quota_from_html(
    provider_id: &str,
    html: &str,
    now_epoch_seconds: i64,
) -> Option<ProviderQuotaSnapshot> {
    let rolling = extract_opencode_go_window(html, "rollingUsage");
    let weekly = extract_opencode_go_window(html, "weeklyUsage");
    let monthly = extract_opencode_go_window(html, "monthlyUsage");

    if rolling.is_none() && weekly.is_none() && monthly.is_none() {
        return None;
    }

    let mut windows = Vec::new();
    if let Some(window) = &rolling {
        windows.push(opencode_go_window(
            "Rolling (5h)",
            ProviderQuotaWindowKind::Rolling,
            window,
            now_epoch_seconds,
        ));
    }
    if let Some(window) = &weekly {
        windows.push(opencode_go_window(
            "Weekly limit",
            ProviderQuotaWindowKind::Weekly,
            window,
            now_epoch_seconds,
        ));
    }
    if let Some(window) = &monthly {
        windows.push(opencode_go_window(
            "Monthly limit",
            ProviderQuotaWindowKind::Monthly,
            window,
            now_epoch_seconds,
        ));
    }

    let primary = shortest_percent_window_used(&windows);
    let plan =
        Some(extract_string_field(html, "subscriptionPlan").unwrap_or_else(|| "Go".to_string()));

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan,
        primary,
        windows,
        credential: None,
        error: None,
    })
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

fn derive_claude_code_snapshot(
    credentials: ClaudeCodeCredentials,
    usage: Result<ClaudeCodeUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    let ClaudeCodeCredentials { plan, source, .. } = credentials;
    match usage {
        Ok(response) => {
            let mut snapshot =
                claude_code_quota_from_usage_response(CLAUDE_CODE_PROVIDER_ID, plan, response);
            snapshot.credential = Some(source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some("Claude Code authorization was rejected; run claude /login".to_string()),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some(error.to_string()),
        },
    }
}

fn derive_codex_snapshot(
    credentials: CodexCredentials,
    usage: Result<CodexUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    let CodexCredentials { plan, source, .. } = credentials;
    match usage {
        Ok(response) => {
            let mut snapshot = codex_quota_from_usage_response(CODEX_PROVIDER_ID, plan, response);
            snapshot.credential = Some(source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: CODEX_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some("Codex authorization was rejected; run codex login to refresh".to_string()),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: CODEX_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan,
            primary: None,
            windows: Vec::new(),
            credential: Some(source),
            error: Some(error.to_string()),
        },
    }
}

fn derive_copilot_snapshot(
    usage: Result<CopilotUsageResponse, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match usage {
        Ok(response) => {
            let mut snapshot = copilot_quota_from_usage_response(COPILOT_PROVIDER_ID, response);
            snapshot.credential = Some("GitHub Copilot token".to_string());
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: COPILOT_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan: None,
            primary: None,
            windows: Vec::new(),
            credential: Some("GitHub Copilot token".to_string()),
            error: Some(
                "GitHub Copilot token was rejected; provide a valid Copilot-scoped token"
                    .to_string(),
            ),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: COPILOT_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan: None,
            primary: None,
            windows: Vec::new(),
            credential: Some("GitHub Copilot token".to_string()),
            error: Some(error.to_string()),
        },
    }
}

fn derive_opencode_go_snapshot(
    credentials: OpenCodeGoCredentials,
    page: Result<String, ProviderQuotaPollError>,
) -> ProviderQuotaSnapshot {
    match page {
        Ok(html) => {
            let Some(mut snapshot) = opencode_go_quota_from_html(
                OPENCODE_GO_PROVIDER_ID,
                &html,
                current_epoch_seconds(),
            ) else {
                return ProviderQuotaSnapshot {
                    provider_id: OPENCODE_GO_PROVIDER_ID.to_string(),
                    status: ProviderQuotaStatus::Failed,
                    plan: Some("Go".to_string()),
                    primary: None,
                    windows: Vec::new(),
                    credential: Some(credentials.source),
                    error: Some(
                        "OpenCode Go page did not contain recognizable usage data".to_string(),
                    ),
                };
            };
            snapshot.credential = Some(credentials.source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => ProviderQuotaSnapshot {
            provider_id: OPENCODE_GO_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Expired,
            plan: Some("Go".to_string()),
            primary: None,
            windows: Vec::new(),
            credential: Some(credentials.source),
            error: Some(
                "OpenCode Go auth cookie expired; copy a fresh auth cookie from opencode.ai"
                    .to_string(),
            ),
        },
        Err(error) => ProviderQuotaSnapshot {
            provider_id: OPENCODE_GO_PROVIDER_ID.to_string(),
            status: ProviderQuotaStatus::Failed,
            plan: Some("Go".to_string()),
            primary: None,
            windows: Vec::new(),
            credential: Some(credentials.source),
            error: Some(error.to_string()),
        },
    }
}

fn derive_configured_provider_snapshot(
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
                return configured_provider_status(
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
        Err(ProviderQuotaPollError::AuthRequired) => configured_provider_status(
            config.provider_id,
            ProviderQuotaStatus::Expired,
            config.plan,
            &credentials.source,
            Some(config.auth_error.to_string()),
        ),
        Err(error) => configured_provider_status(
            config.provider_id,
            ProviderQuotaStatus::Failed,
            config.plan,
            &credentials.source,
            Some(error.to_string()),
        ),
    }
}

fn copilot_window(
    label: &str,
    detail: &CopilotQuotaDetail,
    reset_at: Option<String>,
) -> Option<ProviderQuotaWindow> {
    let remaining = copilot_remaining_percent(detail)?;
    Some(ProviderQuotaWindow {
        label: label.to_string(),
        kind: ProviderQuotaWindowKind::Monthly,
        used: percent_to_u8(100.0 - remaining),
        value_label: None,
        value_only: false,
        reset_at,
        unlimited: detail.unlimited == Some(true),
    })
}

/// Copilot reports remaining quota; an unlimited window has no usable percentage
/// so it is treated as fully available (0% used).
fn copilot_remaining_percent(detail: &CopilotQuotaDetail) -> Option<f64> {
    if detail.unlimited == Some(true) {
        return Some(100.0);
    }
    if let Some(percent) = detail.percent_remaining {
        return Some(percent.clamp(0.0, 100.0));
    }
    let entitlement = detail.entitlement? as f64;
    let remaining = detail.remaining?;
    if entitlement > 0.0 {
        Some((remaining / entitlement * 100.0).clamp(0.0, 100.0))
    } else {
        None
    }
}

struct OpenCodeGoWindow {
    usage_percent: f64,
    reset_in_sec: u64,
}

fn opencode_go_window(
    label: &str,
    kind: ProviderQuotaWindowKind,
    window: &OpenCodeGoWindow,
    now_epoch_seconds: i64,
) -> ProviderQuotaWindow {
    ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(window.usage_percent),
        value_label: None,
        value_only: false,
        reset_at: reset_seconds_to_iso(now_epoch_seconds, window.reset_in_sec),
        unlimited: false,
    }
}

fn extract_opencode_go_window(html: &str, key: &str) -> Option<OpenCodeGoWindow> {
    let obj = object_after_key(html, key)?;
    let usage_percent = number_field(obj, "usagePercent")?;
    let reset_in_sec = number_field(obj, "resetInSec").unwrap_or(0.0);
    Some(OpenCodeGoWindow {
        usage_percent,
        reset_in_sec: reset_in_sec.max(0.0) as u64,
    })
}

fn object_after_key<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}:");
    let mut from = 0;
    while let Some(rel) = s[from..].find(&needle) {
        let after = from + rel + needle.len();
        let rest = &s[after..];
        match rest.chars().next() {
            Some('{') | Some('$') => {
                let open = rest.find('{')?;
                let after_open = &rest[open + 1..];
                let close = after_open.find('}')?;
                return Some(&after_open[..close]);
            }
            _ => from = after,
        }
    }
    None
}

fn number_field(obj: &str, field: &str) -> Option<f64> {
    let needle = format!("{field}:");
    let idx = obj.find(&needle)? + needle.len();
    let token: String = obj[idx..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    token.parse().ok()
}

fn extract_string_field(s: &str, field: &str) -> Option<String> {
    let needle = format!("{field}:");
    let idx = s.find(&needle)? + needle.len();
    let tail = &s[idx..];
    let inner = tail.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn copilot_plan_label(plan: &str) -> String {
    let normalized = plan.to_lowercase();
    if normalized.contains("business") {
        "Copilot Business".to_string()
    } else if normalized.contains("enterprise") {
        "Copilot Enterprise".to_string()
    } else if normalized.contains("free") {
        "Copilot Free".to_string()
    } else if normalized.contains("pro") || normalized.contains("individual") {
        "Copilot Pro".to_string()
    } else if normalized.is_empty() {
        "Copilot".to_string()
    } else {
        format!("Copilot {plan}")
    }
}

/// `quota_reset_date` is a calendar date ("2026-07-01"); anchor a bare date to
/// midnight UTC so the frontend can render the exact month/day.
fn copilot_reset_to_iso(date: &str) -> Option<String> {
    let date = date.trim();
    if date.is_empty() {
        return None;
    }
    if date.contains('T') {
        return Some(date.to_string());
    }
    if is_iso_calendar_date(date) {
        Some(format!("{date}T00:00:00Z"))
    } else {
        None
    }
}

fn is_iso_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && value[0..4].bytes().all(|c| c.is_ascii_digit())
        && value[5..7].bytes().all(|c| c.is_ascii_digit())
        && value[8..10].bytes().all(|c| c.is_ascii_digit())
}

/// Resolve the GitHub token from opencode's `auth.json`. The location honours
/// `OPENCODE_AUTH_FILE`, defaulting to `~/.local/share/opencode/auth.json`.
fn read_opencode_copilot_token() -> Option<String> {
    let content = read_opencode_auth_content()?;
    parse_opencode_copilot_token(&content)
}

/// Extract the `github-copilot` GitHub token from opencode `auth.json` content.
/// OAuth providers store it under `access`; API providers under `key`.
pub fn parse_opencode_copilot_token(content: &str) -> Option<String> {
    parse_opencode_auth_token(content, "github-copilot", &["access", "key"])
}

pub fn parse_opencode_provider_token(content: &str, provider_key: &str) -> Option<String> {
    parse_opencode_auth_token(content, provider_key, &["key", "access"])
}

fn read_opencode_auth_token(provider_key: &str) -> Option<String> {
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

fn codex_window_meta(limit_window_seconds: Option<i64>) -> (ProviderQuotaWindowKind, String) {
    match limit_window_seconds {
        Some(18000) => (ProviderQuotaWindowKind::Rolling, "5-hour limit".to_string()),
        Some(604800) => (ProviderQuotaWindowKind::Weekly, "Weekly limit".to_string()),
        Some(secs) => {
            let hours = secs / 3600;
            let kind = if secs >= 604800 {
                ProviderQuotaWindowKind::Weekly
            } else {
                ProviderQuotaWindowKind::Rolling
            };
            let label = if hours >= 24 {
                format!("{}-day limit", hours / 24)
            } else {
                format!("{}-hour limit", hours)
            };
            (kind, label)
        }
        None => (ProviderQuotaWindowKind::Rolling, "Limit".to_string()),
    }
}

fn unix_seconds_to_iso(secs: i64) -> Option<String> {
    OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
}

fn unix_millis_to_iso(ms: i64) -> Option<String> {
    if ms <= 0 {
        return None;
    }
    unix_seconds_to_iso(ms / 1000)
}

fn reset_seconds_to_iso(now_epoch_seconds: i64, reset_in_sec: u64) -> Option<String> {
    if reset_in_sec == 0 {
        return None;
    }
    let reset_in_sec = i64::try_from(reset_in_sec).ok()?;
    unix_seconds_to_iso(now_epoch_seconds.checked_add(reset_in_sec)?)
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn quota_window(
    label: &str,
    kind: ProviderQuotaWindowKind,
    bucket: ClaudeCodeUsageBucket,
) -> ProviderQuotaWindow {
    ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(bucket.utilization),
        value_label: None,
        value_only: false,
        reset_at: bucket.resets_at,
        unlimited: false,
    }
}

fn percent_to_u8(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, 100.0) as u8
}

fn shortest_percent_window_used(windows: &[ProviderQuotaWindow]) -> Option<u8> {
    let shortest_rank = windows
        .iter()
        .filter(|window| !window.value_only)
        .map(|window| quota_window_kind_rank(&window.kind))
        .min()?;

    windows
        .iter()
        .filter(|window| !window.value_only)
        .filter(|window| quota_window_kind_rank(&window.kind) == shortest_rank)
        .map(|window| window.used)
        .max()
}

fn quota_window_kind_rank(kind: &ProviderQuotaWindowKind) -> u8 {
    match kind {
        ProviderQuotaWindowKind::Rolling => 0,
        ProviderQuotaWindowKind::Weekly => 1,
        ProviderQuotaWindowKind::Monthly => 2,
    }
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build HTTP client")
}

fn format_credit_value(value: f64) -> String {
    format!("{value:.2}")
}

async fn fetch_claude_code_usage(
    access_token: &str,
) -> Result<ClaudeCodeUsageResponse, ProviderQuotaPollError> {
    let response = http_client()
        .get(CLAUDE_CODE_USAGE_URL)
        .bearer_auth(access_token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Claude Code usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<ClaudeCodeUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

#[cfg(target_os = "macos")]
fn read_claude_code_credentials_from_keychain() -> AppResult<Option<ClaudeCodeCredentials>> {
    let output = match Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let content = String::from_utf8(output.stdout)
        .map_err(|error| AppError::Validation(format!("invalid Keychain credentials: {error}")))?;
    parse_claude_code_credentials_from_source(
        content.trim(),
        "macOS Keychain · Claude Code-credentials".to_string(),
    )
}

fn parse_claude_code_credentials(
    content: &str,
    path: PathBuf,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    parse_claude_code_credentials_from_source(content, path_to_display(&path))
}

fn parse_claude_code_credentials_from_source(
    content: &str,
    source: String,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    let json: serde_json::Value = serde_json::from_str(content).map_err(|error| {
        AppError::Validation(format!("invalid Claude Code credentials: {error}"))
    })?;
    let Some(oauth) = json
        .get("claudeAiOauth")
        .or_else(|| json.get("claude.ai_oauth"))
    else {
        return Ok(None);
    };
    let Some(access_token) = oauth.get("accessToken").and_then(|value| value.as_str()) else {
        return Ok(None);
    };
    let expires_at = oauth.get("expiresAt").and_then(|value| value.as_i64());
    let rate_limit_tier = oauth
        .get("rateLimitTier")
        .or_else(|| oauth.get("rate_limit_tier"))
        .and_then(|value| value.as_str());
    let subscription_type = oauth
        .get("subscriptionType")
        .or_else(|| oauth.get("subscription_type"))
        .and_then(|value| value.as_str());

    Ok(Some(ClaudeCodeCredentials {
        access_token: access_token.to_string(),
        expires_at,
        plan: infer_claude_code_plan(rate_limit_tier, subscription_type),
        source,
    }))
}

fn infer_claude_code_plan(
    rate_limit_tier: Option<&str>,
    subscription_type: Option<&str>,
) -> Option<String> {
    let tier = rate_limit_tier.unwrap_or_default().to_lowercase();
    let subscription = subscription_type.unwrap_or_default().to_lowercase();

    for hint in [&subscription, &tier] {
        if hint.contains("max") {
            return Some("Claude Max".to_string());
        }
        if hint.contains("pro") {
            return Some("Claude Pro".to_string());
        }
        if hint.contains("team") {
            return Some("Claude Team".to_string());
        }
        if hint.contains("enterprise") {
            return Some("Claude Enterprise".to_string());
        }
    }

    if subscription.is_empty() && tier.is_empty() {
        None
    } else {
        Some("Claude".to_string())
    }
}

fn is_token_expired(expires_at: Option<i64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    now >= expires_at
}

fn path_to_display(path: &Path) -> String {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return path.to_string_lossy().into_owned();
    };
    match path.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.to_string_lossy()),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn claude_code_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

#[derive(Clone, Debug)]
struct CodexCredentials {
    access_token: String,
    account_id: Option<String>,
    plan: Option<String>,
    source: String,
}

fn parse_codex_credentials(content: &str, path: &Path) -> AppResult<Option<CodexCredentials>> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|error| AppError::Validation(format!("invalid Codex auth.json: {error}")))?;

    let tokens = json.get("tokens");
    let access_token = tokens
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(|value| value.as_str());
    let Some(access_token) = access_token else {
        return Ok(None);
    };
    let account_id = tokens
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let id_token = tokens
        .and_then(|tokens| tokens.get("id_token"))
        .and_then(|value| value.as_str());

    let plan = id_token.and_then(decode_codex_plan_from_id_token);

    Ok(Some(CodexCredentials {
        access_token: access_token.to_string(),
        account_id,
        plan,
        source: path_to_display(path),
    }))
}

fn decode_codex_plan_from_id_token(id_token: &str) -> Option<String> {
    let payload = decode_jwt_payload(id_token)?;
    let plan_type = payload
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(|value| value.as_str())?;
    Some(codex_plan_label(plan_type))
}

fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let mut parts = token.split('.');
    parts.next()?;
    let payload_b64 = parts.next()?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload_b64))
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn codex_plan_label(plan_type: &str) -> String {
    let plan = plan_type.to_lowercase();
    if plan.contains("pro") {
        "ChatGPT Pro".to_string()
    } else if plan.contains("plus") {
        "ChatGPT Plus".to_string()
    } else if plan.contains("team") {
        "ChatGPT Team".to_string()
    } else if plan.contains("enterprise") {
        "ChatGPT Enterprise".to_string()
    } else if plan.contains("business") {
        "ChatGPT Business".to_string()
    } else if plan.is_empty() {
        "ChatGPT".to_string()
    } else {
        format!("ChatGPT {}", plan_type)
    }
}

async fn fetch_codex_usage(
    access_token: &str,
    account_id: Option<&str>,
) -> Result<CodexUsageResponse, ProviderQuotaPollError> {
    let mut request = http_client()
        .get(CODEX_USAGE_URL)
        .bearer_auth(access_token)
        .header("User-Agent", "codex-cli")
        .header("Accept", "application/json");
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Codex usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<CodexUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn codex_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: CODEX_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

async fn fetch_minimax_token_plan_cn_usage(
    api_key: &str,
) -> Result<MiniMaxTokenPlanCnUsageResponse, ProviderQuotaPollError> {
    let response = http_client()
        .get(MINIMAX_TOKEN_PLAN_CN_USAGE_URL)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

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

async fn fetch_configured_provider_usage(
    config: &'static ConfiguredProviderQuotaConfig,
    api_key: &str,
) -> Result<ConfiguredProviderUsageResponse, ProviderQuotaPollError> {
    match config.kind {
        ConfiguredProviderQuotaKind::MiniMaxTokenPlanCn => {
            fetch_minimax_token_plan_cn_usage(api_key)
                .await
                .map(ConfiguredProviderUsageResponse::MiniMaxTokenPlanCn)
        }
        ConfiguredProviderQuotaKind::DeepSeekBalance => fetch_deepseek_balance(api_key)
            .await
            .map(ConfiguredProviderUsageResponse::DeepSeekBalance),
        ConfiguredProviderQuotaKind::OpenRouterCredits => fetch_openrouter_credits(api_key)
            .await
            .map(ConfiguredProviderUsageResponse::OpenRouterCredits),
    }
}

async fn fetch_deepseek_balance(
    api_key: &str,
) -> Result<DeepSeekBalanceResponse, ProviderQuotaPollError> {
    let response = http_client()
        .get(DEEPSEEK_BALANCE_URL)
        .bearer_auth(api_key)
        .header("Host", "api.deepseek.com")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

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
) -> Result<OpenRouterCreditsResponse, ProviderQuotaPollError> {
    let response = http_client()
        .get(OPENROUTER_CREDITS_URL)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

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

async fn fetch_copilot_usage(token: &str) -> Result<CopilotUsageResponse, ProviderQuotaPollError> {
    let response = http_client()
        .get(COPILOT_USAGE_URL)
        .header("Authorization", format!("token {token}"))
        .header("Accept", "application/json")
        .header("Editor-Version", "vscode/1.96.2")
        .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
        .header("User-Agent", "GitHubCopilotChat/0.26.7")
        .header("X-Github-Api-Version", "2025-04-01")
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "Copilot usage endpoint returned {status}"
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<CopilotUsageResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn copilot_status(status: ProviderQuotaStatus, message: &str) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: COPILOT_PROVIDER_ID.to_string(),
        status,
        plan: None,
        primary: None,
        windows: Vec::new(),
        credential: Some(message.to_string()),
        error: None,
    }
}

async fn fetch_opencode_go_page(
    workspace_id: &str,
    auth_cookie: &str,
) -> Result<String, ProviderQuotaPollError> {
    let id = normalize_opencode_go_workspace_id(workspace_id);
    let url = format!("https://opencode.ai/workspace/{id}/go");
    let response = http_client()
        .get(url)
        .header("Cookie", format!("auth={}", auth_cookie.trim()))
        .header("User-Agent", OPENCODE_GO_BROWSER_UA)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    let final_url = response.url().as_str().to_string();
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if final_url.contains("auth.opencode.ai")
        || final_url.contains("/authorize")
        || final_url.contains("/login")
    {
        return Err(ProviderQuotaPollError::AuthRequired);
    }
    if !status.is_success() {
        return Err(ProviderQuotaPollError::Request(format!(
            "OpenCode Go workspace page returned {status}"
        )));
    }

    response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

fn normalize_opencode_go_workspace_id(workspace_id: &str) -> String {
    let workspace_id = workspace_id.trim();
    if workspace_id.starts_with("wrk_") {
        workspace_id.to_string()
    } else {
        format!("wrk_{workspace_id}")
    }
}

fn opencode_go_status(
    status: ProviderQuotaStatus,
    credential: &str,
    error: Option<String>,
) -> ProviderQuotaSnapshot {
    ProviderQuotaSnapshot {
        provider_id: OPENCODE_GO_PROVIDER_ID.to_string(),
        status,
        plan: Some("Go".to_string()),
        primary: None,
        windows: Vec::new(),
        credential: Some(credential.to_string()),
        error,
    }
}

#[derive(Debug, thiserror::Error)]
enum ProviderQuotaPollError {
    #[error("Claude Code authorization failed")]
    AuthRequired,
    #[error("{0}")]
    Request(String),
}
