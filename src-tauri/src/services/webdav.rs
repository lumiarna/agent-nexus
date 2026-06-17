use std::time::Duration;

use reqwest::{Method, RequestBuilder, StatusCode, Url};

use crate::error::{AppError, AppResult};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub type WebdavAuth = Option<(String, Option<String>)>;

pub fn auth_from_credentials(username: &str, password: &str) -> WebdavAuth {
    let username = username.trim();
    if username.is_empty() {
        None
    } else {
        Some((username.to_string(), Some(password.to_string())))
    }
}

pub fn path_segments(raw: &str) -> impl Iterator<Item = &str> {
    raw.trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
}

pub async fn test_connection(base_url: &str, auth: &WebdavAuth) -> AppResult<()> {
    let url = parse_base_url(base_url)?;
    let client = reqwest::Client::new();
    let response = apply_auth(
        client
            .request(method_propfind(), url)
            .header("Depth", "0")
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        auth,
    )
    .send()
    .await
    .map_err(|error| transport_error("connect", base_url, error))?;

    if response.status().is_success() || response.status() == StatusCode::MULTI_STATUS {
        Ok(())
    } else {
        Err(status_error("PROPFIND", response.status(), base_url))
    }
}

pub async fn ensure_remote_directories(
    base_url: &str,
    segments: &[String],
    auth: &WebdavAuth,
) -> AppResult<()> {
    if segments.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::new();
    for depth in 1..=segments.len() {
        let dir_url = directory_url(base_url, &segments[..depth])?;
        let response = apply_auth(
            client
                .request(method_mkcol(), &dir_url)
                .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
            auth,
        )
        .send()
        .await
        .map_err(|error| transport_error("create remote directory", &dir_url, error))?;

        let status = response.status();
        if status == StatusCode::CREATED || status.is_success() {
            continue;
        }

        if matches!(
            status,
            StatusCode::METHOD_NOT_ALLOWED | StatusCode::CONFLICT
        ) && propfind_exists(&client, &dir_url, auth).await?
        {
            continue;
        }

        return Err(status_error("MKCOL", status, &dir_url));
    }

    Ok(())
}

pub fn build_remote_url(base_url: &str, segments: &[String]) -> AppResult<String> {
    let mut url = parse_base_url(base_url)?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| AppError::Validation("invalid WebDAV URL path".to_string()))?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }

    Ok(url.to_string())
}

pub async fn put_bytes(
    url: &str,
    auth: &WebdavAuth,
    bytes: Vec<u8>,
    content_type: &str,
) -> AppResult<()> {
    let client = reqwest::Client::new();
    let response = apply_auth(
        client
            .put(url)
            .header("Content-Type", content_type)
            .body(bytes)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        auth,
    )
    .send()
    .await
    .map_err(|error| transport_error("upload", url, error))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(status_error("PUT", response.status(), url))
    }
}

fn parse_base_url(raw: &str) -> AppResult<Url> {
    let url = Url::parse(raw)
        .map_err(|error| AppError::Validation(format!("invalid WebDAV URL: {error}")))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => Err(AppError::Validation(
            "WebDAV URL must use http or https".to_string(),
        )),
    }
}

fn directory_url(base_url: &str, segments: &[String]) -> AppResult<String> {
    let value = build_remote_url(base_url, segments)?;
    if value.ends_with('/') {
        Ok(value)
    } else {
        Ok(format!("{value}/"))
    }
}

fn method_propfind() -> Method {
    Method::from_bytes(b"PROPFIND").expect("PROPFIND is a valid HTTP method")
}

fn method_mkcol() -> Method {
    Method::from_bytes(b"MKCOL").expect("MKCOL is a valid HTTP method")
}

fn apply_auth(builder: RequestBuilder, auth: &WebdavAuth) -> RequestBuilder {
    match auth {
        Some((username, password)) => builder.basic_auth(username, password.as_deref()),
        None => builder,
    }
}

async fn propfind_exists(
    client: &reqwest::Client,
    url: &str,
    auth: &WebdavAuth,
) -> AppResult<bool> {
    let response = apply_auth(
        client
            .request(method_propfind(), url)
            .header("Depth", "0")
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        auth,
    )
    .send()
    .await
    .map_err(|error| transport_error("verify remote directory", url, error))?;

    Ok(response.status().is_success() || response.status() == StatusCode::MULTI_STATUS)
}

fn status_error(operation: &str, status: StatusCode, url: &str) -> AppError {
    AppError::Internal(format!(
        "WebDAV {operation} failed: {status} ({})",
        redact_url(url)
    ))
}

fn transport_error(operation: &str, url: &str, error: reqwest::Error) -> AppError {
    AppError::Internal(format!(
        "WebDAV {operation} failed: {} ({})",
        error,
        redact_url(url)
    ))
}

fn redact_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.to_string()
        }
        Err(_) => raw.split('?').next().unwrap_or(raw).to_string(),
    }
}
