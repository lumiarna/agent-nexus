use std::{future::Future, pin::Pin, sync::Arc};

use serde::Serialize;

use crate::{
    error::{AppError, AppResult},
    services::{app_config::AppConfigService, outbound_request_log::OutboundRequestLogger},
};

mod claude_auth;
mod shared;
mod providers {
    pub(crate) mod claude_code;
    pub(crate) mod codex;
    pub(crate) mod configured;
    pub(crate) mod copilot;
    pub(crate) mod opencode_custom;
    pub(crate) mod opencode_go;
    pub(crate) mod qoder;
}

pub(crate) use claude_auth::{ClaudeAccessToken, ClaudeAuthError};
pub(crate) use providers::claude_code::{
    ClaudeCodeCredentials, PROVIDER_ID as CLAUDE_CODE_PROVIDER_ID,
};
pub(crate) use providers::codex::{read_credentials as read_codex_credentials, CodexCredentials};
pub use providers::{
    claude_code::{
        claude_code_quota_from_usage_response, ClaudeCodeUsageBucket, ClaudeCodeUsageResponse,
    },
    codex::{
        codex_quota_from_usage_response, codex_reset_credit_windows, CodexRateLimit,
        CodexRateLimitWindow, CodexResetCredit, CodexResetCreditsResponse, CodexUsageResponse,
        CHATGPT_ACCOUNT_ID_HEADER,
    },
    configured::{
        deepseek_balance_quota_from_usage_response,
        minimax_token_plan_cn_quota_from_usage_response,
        openrouter_credits_quota_from_usage_response, DeepSeekBalanceInfo, DeepSeekBalanceResponse,
        MiniMaxTokenPlanCnBaseResp, MiniMaxTokenPlanCnModelRemain, MiniMaxTokenPlanCnUsageResponse,
        OpenRouterCreditsData, OpenRouterCreditsResponse,
    },
    copilot::{
        copilot_quota_from_usage_response, CopilotQuotaDetail, CopilotQuotaSnapshots,
        CopilotUsageResponse,
    },
    opencode_custom::{
        parse_opencode_copilot_token, parse_opencode_custom_providers,
        parse_opencode_provider_token,
    },
    opencode_go::opencode_go_quota_from_html,
    qoder::qoder_quota_from_response,
};
pub(crate) use shared::http_client;
pub use shared::{llm_gateway_quota_from_headers, llm_gateway_quota_from_headers_at};

use providers::{
    claude_code::{ClaudeCodeQuotaAdapter, ClaudeOAuthRefreshResponse},
    codex::CodexQuotaAdapter,
    configured::{
        ConfiguredProviderCredentials, ConfiguredProviderQuotaAdapter,
        ConfiguredProviderQuotaConfig, ConfiguredProviderUsageResponse,
    },
    copilot::CopilotQuotaAdapter,
    opencode_custom::OpenCodeCustomProviderCredentials,
    opencode_go::{OpenCodeGoCredentials, OpenCodeGoQuotaAdapter},
    qoder::{QoderCredentials, QoderQuotaAdapter, QoderUsageResponse},
};

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

type ClaudeCodeUsageFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    providers::claude_code::ClaudeCodeUsageResponse,
                    ProviderQuotaPollError,
                >,
            > + Send
            + 'a,
    >,
>;
type CodexUsageFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<providers::codex::CodexUsageResponse, ProviderQuotaPollError>>
            + Send
            + 'a,
    >,
>;
type CodexResetCreditsFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    providers::codex::CodexResetCreditsResponse,
                    ProviderQuotaPollError,
                >,
            > + Send
            + 'a,
    >,
>;
type CopilotUsageFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<providers::copilot::CopilotUsageResponse, ProviderQuotaPollError>,
            > + Send
            + 'a,
    >,
>;
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
type QoderUsageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<QoderUsageResponse, ProviderQuotaPollError>> + Send + 'a>>;

pub(crate) trait ProviderCredentialSource: Send + Sync {
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
    fn qoder_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<QoderCredentials>>;
    fn configured_provider_credentials(
        &self,
        app_config: &AppConfigService,
        config: &'static ConfiguredProviderQuotaConfig,
    ) -> AppResult<Option<ConfiguredProviderCredentials>>;
    fn opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProviderCredentials>>;
}

pub(crate) trait ProviderUsageTransport: Send + Sync {
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
    fn codex_reset_credits<'a>(
        &'a self,
        access_token: &'a str,
        account_id: Option<&'a str>,
    ) -> CodexResetCreditsFuture<'a>;
    fn copilot_usage<'a>(&'a self, token: &'a str) -> CopilotUsageFuture<'a>;
    fn opencode_go_page<'a>(
        &'a self,
        workspace_id: &'a str,
        auth_cookie: &'a str,
    ) -> OpenCodeGoPageFuture<'a>;
    fn qoder_usage<'a>(&'a self, session_cookie: &'a str) -> QoderUsageFuture<'a>;
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
pub(crate) struct HttpUsageTransport {
    request_logger: OutboundRequestLogger,
}

impl HttpUsageTransport {
    pub(crate) fn new(request_logger: OutboundRequestLogger) -> Self {
        Self { request_logger }
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
static QODER_QUOTA_ADAPTER: QoderQuotaAdapter = QoderQuotaAdapter;
static CONFIGURED_PROVIDER_QUOTA_ADAPTER: ConfiguredProviderQuotaAdapter =
    ConfiguredProviderQuotaAdapter;

#[derive(Clone)]
pub struct ProviderQuotaService {
    app_config: AppConfigService,
    credential_source: Arc<dyn ProviderCredentialSource>,
    usage_transport: Arc<dyn ProviderUsageTransport>,
}

impl ProviderQuotaService {
    pub fn new(app_config: AppConfigService, request_logger: OutboundRequestLogger) -> Self {
        Self {
            app_config,
            credential_source: Arc::new(LocalCredentialSource),
            usage_transport: Arc::new(HttpUsageTransport::new(request_logger)),
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
            return Ok(providers::opencode_custom::quota(
                credentials,
                self.usage_transport.as_ref(),
            )
            .await);
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

fn provider_quota_adapters() -> [&'static dyn ProviderQuotaAdapter; 6] {
    [
        &CLAUDE_CODE_QUOTA_ADAPTER,
        &CODEX_QUOTA_ADAPTER,
        &COPILOT_QUOTA_ADAPTER,
        &OPENCODE_GO_QUOTA_ADAPTER,
        &QODER_QUOTA_ADAPTER,
        &CONFIGURED_PROVIDER_QUOTA_ADAPTER,
    ]
}

impl ProviderCredentialSource for LocalCredentialSource {
    fn claude_code_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<ClaudeCodeCredentials>> {
        providers::claude_code::read_credentials(app_config)
    }

    fn codex_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<CodexCredentials>> {
        providers::codex::read_credentials(app_config)
    }

    fn copilot_token(&self, app_config: &AppConfigService) -> AppResult<Option<String>> {
        providers::copilot::read_token(app_config)
    }

    fn opencode_go_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<OpenCodeGoCredentials>> {
        providers::opencode_go::read_credentials(app_config)
    }

    fn qoder_credentials(
        &self,
        app_config: &AppConfigService,
    ) -> AppResult<Option<QoderCredentials>> {
        providers::qoder::read_credentials(app_config)
    }

    fn configured_provider_credentials(
        &self,
        app_config: &AppConfigService,
        config: &'static ConfiguredProviderQuotaConfig,
    ) -> AppResult<Option<ConfiguredProviderCredentials>> {
        providers::configured::read_credentials(app_config, config)
    }

    fn opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
        providers::opencode_custom::read_credentials()
    }
}

impl ProviderUsageTransport for HttpUsageTransport {
    fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a> {
        Box::pin(providers::claude_code::fetch_usage(
            access_token,
            &self.request_logger,
        ))
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
        Box::pin(providers::claude_code::fetch_oauth_refresh(
            refresh_token,
            &self.request_logger,
        ))
    }

    fn codex_usage<'a>(
        &'a self,
        access_token: &'a str,
        account_id: Option<&'a str>,
    ) -> CodexUsageFuture<'a> {
        Box::pin(providers::codex::fetch_usage(
            access_token,
            account_id,
            &self.request_logger,
        ))
    }

    fn codex_reset_credits<'a>(
        &'a self,
        access_token: &'a str,
        account_id: Option<&'a str>,
    ) -> CodexResetCreditsFuture<'a> {
        Box::pin(providers::codex::fetch_reset_credits(
            access_token,
            account_id,
            &self.request_logger,
        ))
    }

    fn copilot_usage<'a>(&'a self, token: &'a str) -> CopilotUsageFuture<'a> {
        Box::pin(providers::copilot::fetch_usage(token, &self.request_logger))
    }

    fn opencode_go_page<'a>(
        &'a self,
        workspace_id: &'a str,
        auth_cookie: &'a str,
    ) -> OpenCodeGoPageFuture<'a> {
        Box::pin(providers::opencode_go::fetch_page(
            workspace_id,
            auth_cookie,
            &self.request_logger,
        ))
    }

    fn qoder_usage<'a>(&'a self, session_cookie: &'a str) -> QoderUsageFuture<'a> {
        Box::pin(providers::qoder::fetch_usage(
            session_cookie,
            &self.request_logger,
        ))
    }

    fn configured_provider_usage<'a>(
        &'a self,
        config: &'static ConfiguredProviderQuotaConfig,
        api_key: &'a str,
    ) -> ConfiguredProviderUsageFuture<'a> {
        Box::pin(providers::configured::fetch_usage(
            config,
            api_key,
            &self.request_logger,
        ))
    }

    fn opencode_custom_provider_usage<'a>(
        &'a self,
        credentials: &'a OpenCodeCustomProviderCredentials,
    ) -> OpenCodeCustomProviderUsageFuture<'a> {
        Box::pin(providers::opencode_custom::fetch_usage(
            credentials,
            &self.request_logger,
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderQuotaPollError {
    #[error("Claude Code authorization failed")]
    AuthRequired,
    #[error("{0}")]
    Request(String),
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    use super::*;
    use crate::services::provider_quota::QoderUsageFuture;
    struct StaticCredentialSource {
        claude_credentials: Option<ClaudeCodeCredentials>,
    }

    impl ProviderCredentialSource for StaticCredentialSource {
        fn claude_code_credentials(
            &self,
            _app_config: &AppConfigService,
        ) -> AppResult<Option<ClaudeCodeCredentials>> {
            Ok(self.claude_credentials.clone())
        }

        fn codex_credentials(
            &self,
            _app_config: &AppConfigService,
        ) -> AppResult<Option<CodexCredentials>> {
            Ok(None)
        }

        fn copilot_token(&self, _app_config: &AppConfigService) -> AppResult<Option<String>> {
            Ok(None)
        }

        fn opencode_go_credentials(
            &self,
            _app_config: &AppConfigService,
        ) -> AppResult<Option<OpenCodeGoCredentials>> {
            Ok(None)
        }

        fn qoder_credentials(
            &self,
            _app_config: &AppConfigService,
        ) -> AppResult<Option<QoderCredentials>> {
            Ok(None)
        }

        fn configured_provider_credentials(
            &self,
            _app_config: &AppConfigService,
            _config: &'static ConfiguredProviderQuotaConfig,
        ) -> AppResult<Option<ConfiguredProviderCredentials>> {
            Ok(None)
        }

        fn opencode_custom_providers(&self) -> AppResult<Vec<OpenCodeCustomProviderCredentials>> {
            Ok(Vec::new())
        }
    }

    struct FailingClaudeRefreshTransport {
        usage_calls: Arc<AtomicUsize>,
    }

    struct RecordingClaudeTransport {
        usage_results: Mutex<VecDeque<Result<ClaudeCodeUsageResponse, ProviderQuotaPollError>>>,
        refresh_results:
            Mutex<VecDeque<Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>>,
        usage_tokens: Mutex<Vec<String>>,
        refresh_calls: AtomicUsize,
    }

    impl RecordingClaudeTransport {
        fn new(
            usage_results: Vec<Result<ClaudeCodeUsageResponse, ProviderQuotaPollError>>,
            refresh_results: Vec<Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>,
        ) -> Self {
            Self {
                usage_results: Mutex::new(VecDeque::from(usage_results)),
                refresh_results: Mutex::new(VecDeque::from(refresh_results)),
                usage_tokens: Mutex::new(Vec::new()),
                refresh_calls: AtomicUsize::new(0),
            }
        }
    }

    impl ProviderUsageTransport for FailingClaudeRefreshTransport {
        fn claude_code_usage<'a>(&'a self, _access_token: &'a str) -> ClaudeCodeUsageFuture<'a> {
            self.usage_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(ProviderQuotaPollError::Request(
                    "usage should not be called".to_string(),
                ))
            })
        }

        fn claude_code_refresh<'a>(
            &'a self,
            _refresh_token: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Err(ProviderQuotaPollError::AuthRequired) })
        }

        fn codex_usage<'a>(
            &'a self,
            _access_token: &'a str,
            _account_id: Option<&'a str>,
        ) -> CodexUsageFuture<'a> {
            Box::pin(async { unreachable!("codex usage is not part of this test") })
        }

        fn codex_reset_credits<'a>(
            &'a self,
            _access_token: &'a str,
            _account_id: Option<&'a str>,
        ) -> CodexResetCreditsFuture<'a> {
            Box::pin(async { unreachable!("codex reset credits is not part of this test") })
        }

        fn copilot_usage<'a>(&'a self, _token: &'a str) -> CopilotUsageFuture<'a> {
            Box::pin(async { unreachable!("copilot usage is not part of this test") })
        }

        fn opencode_go_page<'a>(
            &'a self,
            _workspace_id: &'a str,
            _auth_cookie: &'a str,
        ) -> OpenCodeGoPageFuture<'a> {
            Box::pin(async { unreachable!("opencode go usage is not part of this test") })
        }

        fn configured_provider_usage<'a>(
            &'a self,
            _config: &'static ConfiguredProviderQuotaConfig,
            _api_key: &'a str,
        ) -> ConfiguredProviderUsageFuture<'a> {
            Box::pin(async { unreachable!("configured provider usage is not part of this test") })
        }

        fn opencode_custom_provider_usage<'a>(
            &'a self,
            _credentials: &'a OpenCodeCustomProviderCredentials,
        ) -> OpenCodeCustomProviderUsageFuture<'a> {
            Box::pin(async { unreachable!("custom provider usage is not part of this test") })
        }

        fn qoder_usage<'a>(&'a self, _session_cookie: &'a str) -> QoderUsageFuture<'a> {
            Box::pin(async { unreachable!("qoder usage is not part of this test") })
        }
    }

    impl ProviderUsageTransport for RecordingClaudeTransport {
        fn claude_code_usage<'a>(&'a self, access_token: &'a str) -> ClaudeCodeUsageFuture<'a> {
            self.usage_tokens
                .lock()
                .expect("lock usage tokens")
                .push(access_token.to_string());
            Box::pin(async move {
                self.usage_results
                    .lock()
                    .expect("lock usage results")
                    .pop_front()
                    .expect("usage result is queued")
            })
        }

        fn claude_code_refresh<'a>(
            &'a self,
            _refresh_token: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ClaudeOAuthRefreshResponse, ProviderQuotaPollError>>
                    + Send
                    + 'a,
            >,
        > {
            self.refresh_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                self.refresh_results
                    .lock()
                    .expect("lock refresh results")
                    .pop_front()
                    .expect("refresh result is queued")
            })
        }

        fn codex_usage<'a>(
            &'a self,
            _access_token: &'a str,
            _account_id: Option<&'a str>,
        ) -> CodexUsageFuture<'a> {
            Box::pin(async { unreachable!("codex usage is not part of this test") })
        }

        fn codex_reset_credits<'a>(
            &'a self,
            _access_token: &'a str,
            _account_id: Option<&'a str>,
        ) -> CodexResetCreditsFuture<'a> {
            Box::pin(async { unreachable!("codex reset credits is not part of this test") })
        }

        fn copilot_usage<'a>(&'a self, _token: &'a str) -> CopilotUsageFuture<'a> {
            Box::pin(async { unreachable!("copilot usage is not part of this test") })
        }

        fn opencode_go_page<'a>(
            &'a self,
            _workspace_id: &'a str,
            _auth_cookie: &'a str,
        ) -> OpenCodeGoPageFuture<'a> {
            Box::pin(async { unreachable!("opencode go usage is not part of this test") })
        }

        fn configured_provider_usage<'a>(
            &'a self,
            _config: &'static ConfiguredProviderQuotaConfig,
            _api_key: &'a str,
        ) -> ConfiguredProviderUsageFuture<'a> {
            Box::pin(async { unreachable!("configured provider usage is not part of this test") })
        }

        fn opencode_custom_provider_usage<'a>(
            &'a self,
            _credentials: &'a OpenCodeCustomProviderCredentials,
        ) -> OpenCodeCustomProviderUsageFuture<'a> {
            Box::pin(async { unreachable!("custom provider usage is not part of this test") })
        }

        fn qoder_usage<'a>(&'a self, _session_cookie: &'a str) -> QoderUsageFuture<'a> {
            Box::pin(async { unreachable!("qoder usage is not part of this test") })
        }
    }

    fn test_app_config() -> AppConfigService {
        let db = Arc::new(crate::database::Database::open_in_memory().expect("open test db"));
        AppConfigService::new(db)
    }

    fn claude_credentials(expires_at: Option<i64>) -> ClaudeCodeCredentials {
        ClaudeCodeCredentials {
            access_token: "old-access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at,
            scopes: vec!["user:profile".to_string()],
            plan: Some("Claude".to_string()),
            source: "test credentials".to_string(),
            credentials_path: None,
            keychain_account: None,
            raw: serde_json::json!({
                "claudeAiOauth": {
                    "accessToken": "old-access-token",
                    "refreshToken": "refresh-token",
                    "expiresAt": expires_at.unwrap_or(4_102_444_800_000_i64)
                }
            }),
        }
    }

    fn usage_response() -> ClaudeCodeUsageResponse {
        ClaudeCodeUsageResponse {
            five_hour: Some(ClaudeCodeUsageBucket {
                utilization: 1.0,
                resets_at: None,
            }),
            seven_day: None,
        }
    }

    fn refresh_response(access_token: &str) -> ClaudeOAuthRefreshResponse {
        ClaudeOAuthRefreshResponse {
            access_token: access_token.to_string(),
            refresh_token: None,
            expires_in: 3600,
        }
    }

    #[tokio::test]
    async fn claude_auth_acquire_refreshes_expiring_token() {
        let app_config = test_app_config();
        let credential_source = StaticCredentialSource {
            claude_credentials: Some(claude_credentials(Some(0))),
        };
        let usage_transport =
            RecordingClaudeTransport::new(Vec::new(), vec![Ok(refresh_response("fresh-token"))]);

        let (_credentials, access_token) =
            ClaudeAccessToken::new(&app_config, &credential_source, &usage_transport)
                .acquire()
                .await
                .expect("acquire access token");

        assert_eq!(access_token, "fresh-token");
        assert_eq!(usage_transport.refresh_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn claude_auth_acquire_reports_refresh_rejection() {
        let app_config = test_app_config();
        let credential_source = StaticCredentialSource {
            claude_credentials: Some(claude_credentials(Some(0))),
        };
        let usage_transport = RecordingClaudeTransport::new(
            Vec::new(),
            vec![Err(ProviderQuotaPollError::AuthRequired)],
        );

        let error = ClaudeAccessToken::new(&app_config, &credential_source, &usage_transport)
            .acquire()
            .await
            .expect_err("refresh rejection is surfaced");

        assert!(matches!(error, ClaudeAuthError::RefreshFailed { .. }));
        assert_eq!(
            error.message(),
            "Claude Code authorization was rejected; run claude /login"
        );
    }

    #[tokio::test]
    async fn claude_auth_retries_once_after_auth_required() {
        let app_config = test_app_config();
        let credentials = claude_credentials(None);
        let credential_source = StaticCredentialSource {
            claude_credentials: Some(credentials.clone()),
        };
        let usage_transport = RecordingClaudeTransport::new(
            vec![
                Err(ProviderQuotaPollError::AuthRequired),
                Ok(usage_response()),
            ],
            vec![Ok(refresh_response("retry-token"))],
        );
        let auth = ClaudeAccessToken::new(&app_config, &credential_source, &usage_transport);
        let usage_transport_ref = &usage_transport;

        let usage = auth
            .with_auth_retry(
                &credentials,
                "old-access-token".to_string(),
                |access_token| async move {
                    usage_transport_ref.claude_code_usage(&access_token).await
                },
                |error| matches!(error, ProviderQuotaPollError::AuthRequired),
            )
            .await
            .expect("retry succeeds");

        assert!(usage.five_hour.is_some());
        assert_eq!(usage_transport.refresh_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            *usage_transport.usage_tokens.lock().expect("lock tokens"),
            vec!["old-access-token".to_string(), "retry-token".to_string()]
        );
    }

    #[tokio::test]
    async fn claude_quota_stops_before_usage_when_expiring_token_refresh_fails() {
        let app_config = test_app_config();
        let usage_calls = Arc::new(AtomicUsize::new(0));
        let credential_source = StaticCredentialSource {
            claude_credentials: Some(claude_credentials(Some(0))),
        };
        let usage_transport = FailingClaudeRefreshTransport {
            usage_calls: usage_calls.clone(),
        };

        let snapshot = ClaudeCodeQuotaAdapter
            .quota_snapshot(&app_config, &credential_source, &usage_transport)
            .await;

        assert_eq!(snapshot.status, ProviderQuotaStatus::Expired);
        assert_eq!(snapshot.credential.as_deref(), Some("test credentials"));
        assert_eq!(usage_calls.load(Ordering::SeqCst), 0);
    }
}
