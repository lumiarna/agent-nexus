use std::time::Duration;

use percent_encoding::percent_decode_str;
use reqwest::{Method, RequestBuilder, StatusCode, Url};
use roxmltree::{Document, Node};
use time::{format_description::well_known::Rfc2822, OffsetDateTime};

use crate::error::{AppError, AppResult};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub type WebdavAuth = Option<(String, Option<String>)>;

#[derive(Debug, Clone, PartialEq)]
pub struct WebdavEntry {
    pub name: String,
    pub is_collection: bool,
    pub content_length: Option<u64>,
    pub last_modified: Option<i64>,
}

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

pub async fn list_directory(
    base_url: &str,
    segments: &[String],
    auth: &WebdavAuth,
) -> AppResult<Vec<WebdavEntry>> {
    let url = directory_url(base_url, segments)?;
    let client = reqwest::Client::new();
    let response = apply_auth(
        client
            .request(method_propfind(), &url)
            .header("Depth", "1")
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        auth,
    )
    .send()
    .await
    .map_err(|error| transport_error("list remote directory", &url, error))?;

    if !(response.status().is_success() || response.status() == StatusCode::MULTI_STATUS) {
        return Err(status_error("PROPFIND", response.status(), &url));
    }

    let body = response
        .text()
        .await
        .map_err(|error| transport_error("read remote directory", &url, error))?;
    parse_multistatus(&url, &body)
}

pub async fn get_bytes(
    base_url: &str,
    segments: &[String],
    auth: &WebdavAuth,
) -> AppResult<Vec<u8>> {
    let url = build_remote_url(base_url, segments)?;
    let client = reqwest::Client::new();
    let response = apply_auth(
        client
            .get(&url)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        auth,
    )
    .send()
    .await
    .map_err(|error| transport_error("download", &url, error))?;

    if !response.status().is_success() {
        return Err(status_error("GET", response.status(), &url));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| transport_error("read download", &url, error))
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

fn parse_multistatus(directory_url: &str, body: &str) -> AppResult<Vec<WebdavEntry>> {
    let directory_url = Url::parse(directory_url)
        .map_err(|error| AppError::Validation(format!("invalid WebDAV URL: {error}")))?;
    let directory_segments = decoded_path_segments(&directory_url)?;
    let document = Document::parse(body)
        .map_err(|error| AppError::Internal(format!("invalid WebDAV multistatus XML: {error}")))?;
    let mut entries = Vec::new();

    for response in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "response")
    {
        let Some(href) = child_text(response, "href") else {
            continue;
        };
        let entry_url = directory_url
            .join(href.trim())
            .map_err(|error| AppError::Internal(format!("invalid WebDAV href: {error}")))?;
        let entry_segments = decoded_path_segments(&entry_url)?;
        if entry_segments == directory_segments {
            continue;
        }
        if !entry_segments.starts_with(&directory_segments)
            || entry_segments.len() != directory_segments.len() + 1
        {
            continue;
        }

        let content_length = child_text(response, "getcontentlength")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<u64>().map_err(|error| {
                    AppError::Internal(format!("invalid WebDAV content length: {error}"))
                })
            })
            .transpose()?;
        let last_modified = child_text(response, "getlastmodified")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(parse_last_modified)
            .transpose()?;

        entries.push(WebdavEntry {
            name: entry_segments
                .last()
                .cloned()
                .ok_or_else(|| AppError::Internal("WebDAV href has no file name".to_string()))?,
            is_collection: response
                .descendants()
                .any(|node| node.is_element() && node.tag_name().name() == "collection"),
            content_length,
            last_modified,
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn child_text<'a>(node: Node<'a, 'a>, tag_name: &str) -> Option<&'a str> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == tag_name)?
        .text()
}

fn decoded_path_segments(url: &Url) -> AppResult<Vec<String>> {
    let segments = url
        .path_segments()
        .ok_or_else(|| AppError::Validation("invalid WebDAV URL path".to_string()))?;
    segments
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            percent_decode_str(segment)
                .decode_utf8()
                .map(|value| value.into_owned())
                .map_err(|error| {
                    AppError::Internal(format!("invalid WebDAV href encoding: {error}"))
                })
        })
        .collect()
}

fn parse_last_modified(value: &str) -> AppResult<i64> {
    OffsetDateTime::parse(value, &Rfc2822)
        .map(|datetime| datetime.unix_timestamp())
        .map_err(|error| AppError::Internal(format!("invalid WebDAV last modified time: {error}")))
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
