use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    error::{AppError, AppResult},
    services::agent_capabilities::{agent_by_name, AgentCapabilitySurface},
};

/// Current Unix time in whole seconds. Shared by every service that timestamps rows.
pub fn now_epoch_seconds() -> AppResult<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::Internal(error.to_string()))?
        .as_secs() as i64)
}

/// Trim `value`, rejecting an empty result with a `{label} is required` validation error.
pub fn required_trimmed<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::Validation(format!("{label} is required")))
    } else {
        Ok(value)
    }
}

/// Resolve an agent capability surface by name, rejecting an unknown name with a
/// validation error. The infallible lookup lives in `agent_capabilities`; this is the
/// shared "must exist" wrapper the asset services need.
pub fn require_agent(name: &str) -> AppResult<&'static AgentCapabilitySurface> {
    agent_by_name(name).ok_or_else(|| AppError::Validation("invalid agent".to_string()))
}
