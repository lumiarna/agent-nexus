use std::future::Future;

use crate::services::app_config::AppConfigService;

use super::{
    providers::claude_code::{is_token_expiring_soon, refresh_and_persist_result},
    ClaudeCodeCredentials, ProviderCredentialSource, ProviderQuotaPollError,
    ProviderUsageTransport,
};

const REQUIRED_SCOPE: &str = "user:profile";
const MISSING_SCOPE_MESSAGE: &str =
    "Claude OAuth token missing 'user:profile' scope. Run 'claude setup-token'.";
const AUTH_REJECTED_MESSAGE: &str = "Claude Code authorization was rejected; run claude /login";

pub(crate) struct ClaudeAccessToken<'a> {
    app_config: &'a AppConfigService,
    credential_source: &'a dyn ProviderCredentialSource,
    usage_transport: &'a dyn ProviderUsageTransport,
}

impl<'a> ClaudeAccessToken<'a> {
    pub(crate) fn new(
        app_config: &'a AppConfigService,
        credential_source: &'a dyn ProviderCredentialSource,
        usage_transport: &'a dyn ProviderUsageTransport,
    ) -> Self {
        Self {
            app_config,
            credential_source,
            usage_transport,
        }
    }

    pub(crate) async fn acquire(&self) -> Result<(ClaudeCodeCredentials, String), ClaudeAuthError> {
        let credentials = read_credentials(self.credential_source, self.app_config)?;

        if !credentials.scopes.is_empty()
            && !credentials
                .scopes
                .iter()
                .any(|scope| scope == REQUIRED_SCOPE)
        {
            return Err(ClaudeAuthError::MissingScope { credentials });
        }

        let mut access_token = credentials.access_token.clone();
        if is_token_expiring_soon(credentials.expires_at) {
            match refresh_and_persist_result(&credentials, self.usage_transport).await {
                Ok(Some(refreshed)) => access_token = refreshed.access_token,
                Ok(None) => {}
                Err(error) => {
                    return Err(ClaudeAuthError::RefreshFailed { credentials, error });
                }
            }
        }

        Ok((credentials, access_token))
    }

    pub(crate) async fn with_auth_retry<T, E, F, Fut>(
        &self,
        credentials: &ClaudeCodeCredentials,
        access_token: String,
        call: F,
        is_auth_required: impl Fn(&E) -> bool,
    ) -> Result<T, E>
    where
        E: From<ClaudeAuthError>,
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let result = call(access_token).await;
        if !matches!(result.as_ref().err(), Some(error) if is_auth_required(error)) {
            return result;
        }

        let refreshed = refresh_and_persist_result(credentials, self.usage_transport)
            .await
            .map_err(|error| ClaudeAuthError::RefreshFailed {
                credentials: credentials.clone(),
                error,
            })?
            .ok_or_else(|| ClaudeAuthError::RefreshRejected {
                credentials: credentials.clone(),
            })?;

        call(refreshed.access_token).await
    }
}

fn read_credentials(
    credential_source: &dyn ProviderCredentialSource,
    app_config: &AppConfigService,
) -> Result<ClaudeCodeCredentials, ClaudeAuthError> {
    credential_source
        .claude_code_credentials(app_config)
        .map_err(|error| ClaudeAuthError::Terminal(error.to_string()))?
        .ok_or(ClaudeAuthError::NoCreds)
}

#[derive(Debug)]
pub(crate) enum ClaudeAuthError {
    NoCreds,
    MissingScope {
        credentials: ClaudeCodeCredentials,
    },
    RefreshFailed {
        credentials: ClaudeCodeCredentials,
        error: ProviderQuotaPollError,
    },
    RefreshRejected {
        credentials: ClaudeCodeCredentials,
    },
    Terminal(String),
}

impl ClaudeAuthError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::NoCreds => "Claude Code credentials were not found".to_string(),
            Self::MissingScope { .. } => MISSING_SCOPE_MESSAGE.to_string(),
            Self::RefreshFailed { error, .. } => match error {
                ProviderQuotaPollError::AuthRequired => AUTH_REJECTED_MESSAGE.to_string(),
                ProviderQuotaPollError::Request(message) => message.clone(),
            },
            Self::RefreshRejected { .. } => AUTH_REJECTED_MESSAGE.to_string(),
            Self::Terminal(message) => message.clone(),
        }
    }
}

impl From<ClaudeAuthError> for ProviderQuotaPollError {
    fn from(error: ClaudeAuthError) -> Self {
        match error {
            ClaudeAuthError::RefreshFailed { error, .. } => error,
            ClaudeAuthError::RefreshRejected { .. } => ProviderQuotaPollError::AuthRequired,
            ClaudeAuthError::NoCreds
            | ClaudeAuthError::MissingScope { .. }
            | ClaudeAuthError::Terminal(_) => ProviderQuotaPollError::Request(error.message()),
        }
    }
}
