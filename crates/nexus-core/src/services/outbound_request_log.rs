use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use reqwest::RequestBuilder;
use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use url::Url;

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct OutboundRequestLogger {
    root: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug)]
pub struct OutboundRequestContext {
    pub category: &'static str,
    pub operation: &'static str,
    pub provider_id: Option<String>,
    pub method: &'static str,
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OutboundRequestError {
    #[error("{0}")]
    Http(reqwest::Error),
    #[error("{0}")]
    Log(AppError),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutboundRequestLogRecord {
    ts: String,
    category: &'static str,
    operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_id: Option<String>,
    method: &'static str,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    duration_ms: u64,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl OutboundRequestLogger {
    pub fn from_app_data_dir(app_data_dir: impl AsRef<Path>) -> AppResult<Self> {
        Self::from_log_root(app_data_dir.as_ref().join("logs").join("outbound-requests"))
    }

    pub fn from_log_root(root: impl Into<PathBuf>) -> AppResult<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root: Arc::new(root),
            lock: Arc::new(Mutex::new(())),
        })
    }

    #[doc(hidden)]
    pub fn for_test() -> AppResult<Self> {
        let root = std::env::temp_dir()
            .join("agent-nexus-test-outbound-requests")
            .join(std::process::id().to_string());
        Self::from_log_root(root)
    }

    pub async fn send(
        &self,
        builder: RequestBuilder,
        context: OutboundRequestContext,
    ) -> Result<reqwest::Response, OutboundRequestError> {
        let started = Instant::now();
        let response = builder.send().await;
        let duration_ms = duration_ms(started);

        match response {
            Ok(response) => {
                let final_url = response.url().as_str();
                let status = response.status();
                let result = if status.is_success() {
                    "ok"
                } else {
                    "http_error"
                };
                self.write_record(OutboundRequestLogRecord {
                    ts: timestamp(),
                    category: context.category,
                    operation: context.operation,
                    provider_id: context.provider_id,
                    method: context.method,
                    host: host(&context.url),
                    final_url: final_url_for_record(&context.url, final_url),
                    url: redact_url(&context.url),
                    status: Some(status.as_u16()),
                    duration_ms,
                    result,
                    error: None,
                })
                .map_err(OutboundRequestError::Log)?;
                Ok(response)
            }
            Err(error) => {
                let message = error.to_string();
                self.write_record(OutboundRequestLogRecord {
                    ts: timestamp(),
                    category: context.category,
                    operation: context.operation,
                    provider_id: context.provider_id,
                    method: context.method,
                    host: host(&context.url),
                    final_url: None,
                    url: redact_url(&context.url),
                    status: None,
                    duration_ms,
                    result: "transport_error",
                    error: Some(message),
                })
                .map_err(OutboundRequestError::Log)?;
                Err(OutboundRequestError::Http(error))
            }
        }
    }

    fn write_record(&self, record: OutboundRequestLogRecord) -> AppResult<()> {
        let now = OffsetDateTime::now_utc();
        let path = self.root.join(format!(
            "{:04}-{:02}-{:02}.ndjson",
            now.year(),
            u8::from(now.month()),
            now.day()
        ));
        let line = serde_json::to_string(&record)
            .map_err(|error| AppError::Internal(format!("serialize request log: {error}")))?;

        let _guard = self
            .lock
            .lock()
            .map_err(|error| AppError::Internal(format!("lock request log: {error}")))?;
        fs::create_dir_all(&*self.root)?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

fn timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("Rfc3339 can format UTC timestamps")
}

fn duration_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

fn host(raw: &str) -> Option<String> {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
}

fn final_url_for_record(request_url: &str, final_url: &str) -> Option<String> {
    let request = redact_url(request_url);
    let final_value = redact_url(final_url);
    if request == final_value {
        None
    } else {
        Some(final_value)
    }
}

fn redact_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => raw.split('?').next().unwrap_or(raw).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    #[tokio::test]
    async fn writes_sanitized_ndjson_record_for_http_request() {
        let root = tempfile::tempdir().expect("create temp dir");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
        let url = format!(
            "http://user:secret@{}/quota?token=hidden",
            listener.local_addr().expect("server addr")
        );
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0; 2048];
            let _ = stream.read(&mut buffer).expect("read request");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .expect("write response");
        });

        let logger =
            OutboundRequestLogger::from_log_root(root.path()).expect("create request logger");
        let response = logger
            .send(
                reqwest::Client::new().get(&url),
                OutboundRequestContext {
                    category: "provider_quota",
                    operation: "test",
                    provider_id: Some("example".to_string()),
                    method: "GET",
                    url,
                },
            )
            .await
            .expect("send request");
        assert!(response.status().is_success());
        server.join().expect("join server");

        let log_file = fs::read_dir(root.path())
            .expect("read log dir")
            .next()
            .expect("log file")
            .expect("log entry")
            .path();
        let content = fs::read_to_string(log_file).expect("read log file");
        assert!(content.contains("\"providerId\":\"example\""));
        assert!(!content.contains("secret"));
        assert!(!content.contains("token=hidden"));
    }
}
