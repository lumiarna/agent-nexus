use std::{
    collections::BTreeMap,
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
use time::{format_description::well_known::Rfc3339, OffsetDateTime, Time};

use crate::{
    error::{AppError, AppResult},
    services::app_config::{AppConfigService, OpenCodeGoConnectionParams},
};

pub(crate) const CLAUDE_CODE_PROVIDER_ID: &str = "claude";
const CLAUDE_CODE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_CODE_KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const CLAUDE_OAUTH_REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";

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

const OPENCODE_CONFIG_FILE_ENV: &str = "OPENCODE_CONFIG_FILE";
const OPENCODE_CONFIG_DIR_ENV: &str = "OPENCODE_CONFIG_DIR";
const OPENAI_COMPATIBLE_NPM: &str = "@ai-sdk/openai-compatible";
const OPENAI_NPM: &str = "@ai-sdk/openai";
const OPENCODE_CUSTOM_PROVIDER_PLAN: &str = "OpenCode custom";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeCustomProvider {
    pub id: String,
    pub name: String,
    pub npm: String,
    pub base_url: String,
    pub model_id: String,
}

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
struct OpenCodeCustomProviderCredentials {
    provider: OpenCodeCustomProvider,
    api_key: String,
    source: String,
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
pub(crate) struct ClaudeCodeCredentials {
    pub(crate) access_token: String,
    refresh_token: Option<String>,
    pub(crate) expires_at: Option<i64>,
    pub(crate) scopes: Vec<String>,
    plan: Option<String>,
    pub(crate) source: String,
    credentials_path: Option<PathBuf>,
    keychain_account: Option<String>,
    raw: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ClaudeOAuthRefreshResponse {
    pub(crate) access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
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
type OpenCodeCustomProviderUsageFuture<'a> = Pin<
    Box<dyn Future<Output = Result<Vec<(String, String)>, ProviderQuotaPollError>> + Send + 'a>,
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
    fn opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProviderCredentials>>;
}

trait ProviderUsageTransport: Send + Sync {
    fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a>;
    fn claude_code_refresh<'a>(
        &'a self,
        refresh_token: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>
                + Send
                + 'a,
        >,
    >;
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
    fn opencode_custom_provider_usage<'a>(
        &'a self,
        credentials: &'a OpenCodeCustomProviderCredentials,
    ) -> OpenCodeCustomProviderUsageFuture<'a>;
}

#[derive(Clone)]
pub(crate) struct LocalCredentialSource;

#[derive(Clone)]
pub(crate) struct HttpUsageTransport;

impl LocalCredentialSource {
    pub(crate) fn claude_code_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<ClaudeCodeCredentials>> {
        <Self as ProviderCredentialSource>::claude_code_credentials(self, app_config)
    }
}

impl HttpUsageTransport {
    pub(crate) async fn refresh_claude_code_credentials(
        &self,
        credentials: &ClaudeCodeCredentials,
    ) -> Option<String> {
        refresh_and_persist(credentials, self)
            .await
            .map(|refreshed| refreshed.access_token)
    }
}

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

        let custom_provider = self
            .credential_source
            .opencode_custom_providers()?
            .into_iter()
            .find(|credentials| credentials.provider.id == provider_id);
        if let Some(credentials) = custom_provider {
            return Ok(
                opencode_custom_provider_quota(credentials, self.usage_transport.as_ref()).await,
            );
        }
        Err(AppError::Validation("unsupported provider".to_string()))
    }

    pub fn list_opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProvider>> {
        Ok(self
            .credential_source
            .opencode_custom_providers()?
            .into_iter()
            .map(|credentials| credentials.provider)
            .collect())
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

    fn opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
        read_opencode_custom_provider_credentials()
    }
}

impl ProviderUsageTransport for HttpUsageTransport {
    fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a> {
        Box::pin(fetch_claude_code_usage(access_token))
    }

    fn claude_code_refresh<'a>(
        &'a self,
        refresh_token: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(fetch_claude_oauth_refresh(refresh_token))
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

    fn opencode_custom_provider_usage<'a>(
        &'a self,
        credentials: &'a OpenCodeCustomProviderCredentials,
    ) -> OpenCodeCustomProviderUsageFuture<'a> {
        Box::pin(fetch_opencode_custom_provider_usage(credentials))
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

        if !credentials.scopes.is_empty()
            && !credentials
                .scopes
                .iter()
                .any(|scope| scope == "user:profile")
        {
            return ProviderQuotaSnapshot {
                provider_id: CLAUDE_CODE_PROVIDER_ID.to_string(),
                status: ProviderQuotaStatus::Failed,
                plan: credentials.plan,
                primary: None,
                windows: Vec::new(),
                credential: Some(credentials.source.clone()),
                error: Some(
                    "Claude OAuth token missing 'user:profile' scope. Run 'claude setup-token'."
                        .to_string(),
                ),
            };
        }

        let mut access_token = credentials.access_token.clone();
        if is_token_expiring_soon(credentials.expires_at) {
            if let Some(refreshed) = refresh_and_persist(&credentials, usage_transport).await {
                access_token = refreshed.access_token.clone();
            }
        }

        let mut usage = usage_transport.claude_code_usage(&access_token).await;
        if let Err(ProviderQuotaPollError::AuthRequired) = usage {
            if let Some(refreshed) = refresh_and_persist(&credentials, usage_transport).await {
                access_token = refreshed.access_token.clone();
                usage = usage_transport.claude_code_usage(&access_token).await;
            }
        }

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

async fn opencode_custom_provider_quota(
    credentials: OpenCodeCustomProviderCredentials,
    usage_transport: &dyn ProviderUsageTransport,
) -> ProviderQuotaSnapshot {
    if credentials.api_key.is_empty() {
        return configured_provider_status(
            &credentials.provider.id,
            ProviderQuotaStatus::NoCreds,
            OPENCODE_CUSTOM_PROVIDER_PLAN,
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
            let Some(mut snapshot) = llm_gateway_quota_from_headers(
                &credentials.provider.id,
                OPENCODE_CUSTOM_PROVIDER_PLAN,
                &borrowed_headers,
            ) else {
                return configured_provider_status(
                    &credentials.provider.id,
                    ProviderQuotaStatus::Failed,
                    OPENCODE_CUSTOM_PROVIDER_PLAN,
                    &credentials.source,
                    Some("response did not contain token quota headers".to_string()),
                );
            };
            snapshot.credential = Some(credentials.source);
            snapshot
        }
        Err(ProviderQuotaPollError::AuthRequired) => configured_provider_status(
            &credentials.provider.id,
            ProviderQuotaStatus::Expired,
            OPENCODE_CUSTOM_PROVIDER_PLAN,
            &credentials.source,
            Some("OpenCode custom provider API key was rejected".to_string()),
        ),
        Err(error) => configured_provider_status(
            &credentials.provider.id,
            ProviderQuotaStatus::Failed,
            OPENCODE_CUSTOM_PROVIDER_PLAN,
            &credentials.source,
            Some(error.to_string()),
        ),
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

pub fn parse_opencode_custom_providers(content: &str) -> AppResult<Vec<OpenCodeCustomProvider>> {
    Ok(parse_opencode_custom_provider_credentials(content)?
        .into_iter()
        .map(|credentials| credentials.provider)
        .collect())
}

fn parse_opencode_custom_provider_credentials(
    content: &str,
) -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
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

fn read_opencode_custom_provider_credentials() -> AppResult<Vec<OpenCodeCustomProviderCredentials>>
{
    let Some(path) = opencode_config_file_path() else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    parse_opencode_custom_provider_credentials(&fs::read_to_string(path)?)
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

pub fn llm_gateway_quota_from_headers(
    provider_id: &str,
    plan: &str,
    headers: &[(&str, &str)],
) -> Option<ProviderQuotaSnapshot> {
    llm_gateway_quota_from_headers_at(provider_id, plan, headers, current_epoch_seconds())
}

pub fn llm_gateway_quota_from_headers_at(
    provider_id: &str,
    plan: &str,
    headers: &[(&str, &str)],
    now_epoch_seconds: i64,
) -> Option<ProviderQuotaSnapshot> {
    let values = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .trim()
                .parse::<u64>()
                .ok()
                .map(|parsed| (name.to_ascii_lowercase(), parsed))
        })
        .collect::<BTreeMap<_, _>>();

    let windows = [
        gateway_quota_window(
            &values,
            "Minute limit",
            ProviderQuotaWindowKind::Rolling,
            &["x-token-count-limit-per-minute"],
            "x-token-count-used-per-minute",
            None,
        ),
        gateway_quota_window(
            &values,
            "Hourly limit",
            ProviderQuotaWindowKind::Rolling,
            &[
                "x-token-count-limit-per-hour-and-user",
                "x-token-count-limit-per-hour-and-client-id",
            ],
            "x-token-count-used-per-hour",
            None,
        ),
        gateway_quota_window(
            &values,
            "Daily limit",
            ProviderQuotaWindowKind::Rolling,
            &[
                "x-token-count-limit-per-day-and-user",
                "x-token-count-limit-per-day-and-client-id",
            ],
            "x-token-count-used-per-day",
            None,
        ),
        gateway_quota_window(
            &values,
            "Monthly limit",
            ProviderQuotaWindowKind::Monthly,
            &[
                "x-token-count-limit-per-month-and-user",
                "x-token-count-limit-per-month-and-client-id",
            ],
            "x-token-count-used-per-month",
            next_natural_month_reset_at(now_epoch_seconds),
        ),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if windows.is_empty() {
        return None;
    }

    Some(ProviderQuotaSnapshot {
        provider_id: provider_id.to_string(),
        status: ProviderQuotaStatus::Available,
        plan: Some(plan.to_string()),
        primary: windows.first().map(|window| window.used),
        windows,
        credential: None,
        error: None,
    })
}

fn gateway_quota_window(
    headers: &BTreeMap<String, u64>,
    label: &str,
    kind: ProviderQuotaWindowKind,
    limit_headers: &[&str],
    used_header: &str,
    reset_at: Option<String>,
) -> Option<ProviderQuotaWindow> {
    let limit = limit_headers
        .iter()
        .filter_map(|name| headers.get(*name).copied())
        .min()?;
    if limit == 0 {
        return None;
    }
    let used_tokens = headers.get(used_header).copied()?;

    Some(ProviderQuotaWindow {
        label: label.to_string(),
        kind,
        used: percent_to_u8(used_tokens as f64 / limit as f64 * 100.0),
        value_label: Some(format!(
            "{} / {} tokens",
            format_token_count(used_tokens),
            format_token_count(limit)
        )),
        value_only: false,
        reset_at,
        unlimited: false,
    })
}

fn next_natural_month_reset_at(now_epoch_seconds: i64) -> Option<String> {
    let current_month_start = OffsetDateTime::from_unix_timestamp(now_epoch_seconds)
        .ok()?
        .replace_day(1)
        .ok()?
        .replace_time(Time::MIDNIGHT);
    let next_month = current_month_start
        .checked_add(time::Duration::days(32))?
        .replace_day(1)
        .ok()?;
    next_month.format(&Rfc3339).ok()
}

fn format_token_count(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
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

pub(crate) fn http_client() -> reqwest::Client {
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

async fn fetch_claude_oauth_refresh(
    refresh_token: &str,
) -> Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLAUDE_OAUTH_CLIENT_ID),
    ];
    let response = http_client()
        .post(CLAUDE_OAUTH_REFRESH_URL)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;

    if !response.status().is_success() {
        return Err(ProviderQuotaPollError::AuthRequired);
    }

    let body = response
        .text()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
    serde_json::from_str::<ClaudeOAuthRefreshResponse>(&body)
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))
}

/// Refresh the Claude OAuth access token via the refresh-token grant and persist
/// the refreshed credentials back to their source (file or macOS Keychain).
async fn refresh_and_persist(
    credentials: &ClaudeCodeCredentials,
    usage_transport: &dyn ProviderUsageTransport,
) -> Option<ClaudeOAuthRefreshResponse> {
    let refresh_token = credentials.refresh_token.as_deref()?;
    match usage_transport.claude_code_refresh(refresh_token).await {
        Ok(refreshed) => {
            persist_refreshed_credentials(credentials, &refreshed);
            Some(refreshed)
        }
        Err(_) => None,
    }
}

fn persist_refreshed_credentials(
    credentials: &ClaudeCodeCredentials,
    refreshed: &ClaudeOAuthRefreshResponse,
) {
    let mut raw = credentials.raw.clone();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let new_expires_at = now_ms + refreshed.expires_in.saturating_mul(1000);

    let oauth = if let Some(oauth) = raw.get_mut("claudeAiOauth") {
        oauth
    } else if let Some(oauth) = raw.get_mut("claude.ai_oauth") {
        oauth
    } else {
        return;
    };
    oauth["accessToken"] = serde_json::json!(refreshed.access_token);
    if let Some(refresh_token) = &refreshed.refresh_token {
        oauth["refreshToken"] = serde_json::json!(refresh_token);
    }
    oauth["expiresAt"] = serde_json::json!(new_expires_at);

    if let Some(account) = &credentials.keychain_account {
        #[cfg(target_os = "macos")]
        {
            // Compact JSON — macOS `security -w` hex-encodes values containing
            // newlines, which Claude Code cannot read back. Avoid newlines.
            write_keychain_password(CLAUDE_CODE_KEYCHAIN_SERVICE, account, &raw.to_string());
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = account;
        }
    } else if let Some(path) = &credentials.credentials_path {
        let pretty = serde_json::to_string_pretty(&raw).unwrap_or_else(|_| raw.to_string());
        let _ = fs::write(path, format!("{pretty}\n"));
    }
}

#[cfg(target_os = "macos")]
fn read_keychain_account(service: &str) -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", service, "-g"])
        .stderr(std::process::Stdio::piped())
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    extract_keychain_account(&stderr)
}

fn extract_keychain_account(text: &str) -> Option<String> {
    let needle = "\"acct\"<blob>=\"";
    let start = text.find(needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    let account = &rest[..end];
    if account.is_empty() {
        None
    } else {
        Some(account.to_string())
    }
}

#[cfg(target_os = "macos")]
fn write_keychain_password(service: &str, account: &str, value: &str) {
    let _ = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            account,
            "-s",
            service,
            "-w",
            value,
        ])
        .output();
}

#[cfg(target_os = "macos")]
fn read_claude_code_credentials_from_keychain() -> AppResult<Option<ClaudeCodeCredentials>> {
    let output = match Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            CLAUDE_CODE_KEYCHAIN_SERVICE,
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
    let account = read_keychain_account(CLAUDE_CODE_KEYCHAIN_SERVICE);
    parse_claude_code_credentials_from_source(
        content.trim(),
        format!("macOS Keychain · {CLAUDE_CODE_KEYCHAIN_SERVICE}"),
        None,
        account,
    )
}

fn parse_claude_code_credentials(
    content: &str,
    path: PathBuf,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    parse_claude_code_credentials_from_source(content, path_to_display(&path), Some(path), None)
}

fn parse_claude_code_credentials_from_source(
    content: &str,
    source: String,
    credentials_path: Option<PathBuf>,
    keychain_account: Option<String>,
) -> AppResult<Option<ClaudeCodeCredentials>> {
    let Some(json) = try_parse_credential_json(content) else {
        return Ok(None);
    };
    let Some(oauth) = json
        .get("claudeAiOauth")
        .or_else(|| json.get("claude.ai_oauth"))
    else {
        return Ok(None);
    };
    let Some(access_token) = oauth.get("accessToken").and_then(|value| value.as_str()) else {
        return Ok(None);
    };
    let refresh_token = oauth
        .get("refreshToken")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let expires_at = oauth.get("expiresAt").and_then(|value| value.as_i64());
    let scopes = oauth
        .get("scopes")
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
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
        refresh_token,
        expires_at,
        scopes,
        plan: infer_claude_code_plan(rate_limit_tier, subscription_type),
        source,
        credentials_path,
        keychain_account,
        raw: json,
    }))
}

/// Parse a Claude Code credential document, tolerating macOS Keychain's
/// hex-encoding of values that contain newlines.
fn try_parse_credential_json(text: &str) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(value);
    }
    try_decode_hex_json(text)
}

fn try_decode_hex_json(text: &str) -> Option<serde_json::Value> {
    let hex = text.trim();
    let hex = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    serde_json::from_slice(&bytes).ok()
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

pub(crate) fn is_token_expiring_soon(expires_at: Option<i64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    now >= expires_at - 60_000
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

async fn fetch_opencode_custom_provider_usage(
    credentials: &OpenCodeCustomProviderCredentials,
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

    let response = http_client()
        .post(&endpoint)
        .bearer_auth(&credentials.api_key)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|error| ProviderQuotaPollError::Request(error.to_string()))?;
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
pub(crate) enum ProviderQuotaPollError {
    #[error("Claude Code authorization failed")]
    AuthRequired,
    #[error("{0}")]
    Request(String),
}
