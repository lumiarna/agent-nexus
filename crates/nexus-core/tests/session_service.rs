use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use nexus_core::{
    database::Database,
    services::{
        projects::ProjectService,
        sessions::SessionService,
        sync::{SyncService, WebdavSettingsInput},
    },
};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn http_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn spawn_webdav_server(
    responses: Vec<String>,
) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test webdav server");
    let url = format!("http://{}/webdav/", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_thread = Arc::clone(&requests);

    let handle = std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept webdav request");
            let request = read_http_request(&mut stream);
            let raw = String::from_utf8_lossy(&request);
            requests_for_thread
                .lock()
                .expect("lock request log")
                .push(raw.to_string());
            stream
                .write_all(response.as_bytes())
                .expect("write webdav response");
        }
    });

    (url, requests, handle)
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let size = stream.read(&mut buffer).expect("read webdav request");
        if size == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..size]);

        let Some(header_end) = find_header_end(&data) else {
            continue;
        };
        let header = String::from_utf8_lossy(&data[..header_end]);
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        if data.len() >= header_end + 4 + content_length {
            break;
        }
    }

    data
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|window| window == b"\r\n\r\n")
}

fn cloud_session_body() -> &'static str {
    r#"---
name: Cloud Session 接线
description: 从 Cloud 聚合会话索引并按需读取正文。
created: 2026-06-25T21:10:37
updated: 2026-06-25T21:10:37
---

# Cloud Session 接线

## 设计决策

- Cloud Session 来自 WebDAV 目录聚合。
"#
}

fn cloud_listing(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/webdav/agent-nexus-sync/Session/agent-nexus/</d:href>
    <d:propstat>
      <d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/webdav/agent-nexus-sync/Session/agent-nexus/260625-cloud.md</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype/>
        <d:getcontentlength>{}</d:getcontentlength>
        <d:getlastmodified>Thu, 25 Jun 2026 13:10:37 GMT</d:getlastmodified>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#,
        body.len()
    )
}

#[test]
fn scans_local_project_session_markdown_files() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sessions = SessionService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    let project = projects
        .record_project(repo.clone())
        .expect("record project");
    let session_dir = Path::new(&repo).join("__sessions");
    fs::create_dir_all(&session_dir).expect("create session dir");
    fs::write(
        session_dir.join("260618-2208-Session本地数据接入.md"),
        r#"---
name: Session本地数据接入
description: 给 Session 页面接入真实 Local Session 数据。
created: 2026-06-18T22:08:31
updated: 2026-06-18T22:08:31
---

# Session 本地数据接入

## 设计决策

- Local Session 来自 Project 的本地会话目录。
"#,
    )
    .expect("write session");
    fs::write(session_dir.join("scratch.txt"), "ignore me").expect("write non-session file");

    let rows = sessions.scan_local_sessions().expect("scan local sessions");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Session本地数据接入");
    assert_eq!(rows[0].project, project.id);
    assert_eq!(rows[0].project_name, "agent-nexus");
    assert_eq!(
        rows[0].file,
        "__sessions/260618-2208-Session本地数据接入.md"
    );
    assert_eq!(rows[0].source, "local");
    assert_eq!(
        rows[0].excerpt,
        "给 Session 页面接入真实 Local Session 数据。"
    );
    assert_eq!(rows[0].body, "");

    let detail = sessions
        .get_local_session(rows[0].id.clone())
        .expect("get local session detail");

    assert!(detail.body.starts_with("# Session 本地数据接入"));
    assert!(!detail.body.contains("created: 2026-06-18T22:08:31"));
}

#[tokio::test]
async fn scans_cloud_project_session_markdown_files_from_webdav() {
    let body = cloud_session_body();
    let listing = cloud_listing(body);
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("207 Multi-Status", &listing),
        http_response("200 OK", body),
        http_response("200 OK", body),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone());
    let sessions = SessionService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    let project = projects.record_project(repo).expect("record project");
    sync.save_webdav_settings(WebdavSettingsInput {
        url,
        user: String::new(),
        pass: String::new(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");

    let rows = sessions
        .scan_cloud_sessions()
        .await
        .expect("scan cloud sessions");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Cloud Session 接线");
    assert_eq!(rows[0].project, project.id);
    assert_eq!(rows[0].project_name, "agent-nexus");
    assert_eq!(rows[0].file, "__sessions/260625-cloud.md");
    assert_eq!(rows[0].source, "cloud");
    assert_eq!(rows[0].excerpt, "从 Cloud 聚合会话索引并按需读取正文。");
    assert_eq!(rows[0].body, "");

    let detail = sessions
        .get_cloud_session(rows[0].id.clone())
        .await
        .expect("get cloud session detail");

    assert!(detail.body.starts_with("# Cloud Session 接线"));
    assert!(!detail.body.contains("created: 2026-06-25T21:10:37"));

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(requests[0].starts_with("PROPFIND /webdav/agent-nexus-sync/Session/agent-nexus/"));
    assert!(requests[0].contains("Depth: 1") || requests[0].contains("depth: 1"));
    assert!(
        requests[1].starts_with("GET /webdav/agent-nexus-sync/Session/agent-nexus/260625-cloud.md")
    );
    assert!(
        requests[2].starts_with("GET /webdav/agent-nexus-sync/Session/agent-nexus/260625-cloud.md")
    );
}

#[tokio::test]
async fn scan_cloud_sessions_reuses_cached_metadata_for_unchanged_files() {
    let body = cloud_session_body();
    let listing = cloud_listing(body);
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("207 Multi-Status", &listing),
        http_response("200 OK", body),
        http_response("207 Multi-Status", &listing),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone());
    let sessions = SessionService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    projects.record_project(repo).expect("record project");
    sync.save_webdav_settings(WebdavSettingsInput {
        url,
        user: String::new(),
        pass: String::new(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");

    let first = sessions
        .scan_cloud_sessions()
        .await
        .expect("first cloud scan");
    let second = sessions
        .scan_cloud_sessions()
        .await
        .expect("second cloud scan");

    assert_eq!(first, second);

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    let get_count = requests
        .iter()
        .filter(|request| request.starts_with("GET "))
        .count();
    assert_eq!(get_count, 1);
}

#[cfg(unix)]
#[test]
fn scan_local_sessions_does_not_follow_directory_symlinks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sessions = SessionService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    projects
        .record_project(repo.clone())
        .expect("record project");
    let session_dir = Path::new(&repo).join("__sessions");
    fs::create_dir_all(&session_dir).expect("create session dir");
    fs::write(session_dir.join("local.md"), "# Local\n").expect("write local session");
    std::os::unix::fs::symlink(Path::new(&repo), session_dir.join("repo-loop"))
        .expect("create symlink loop");

    let rows = sessions.scan_local_sessions().expect("scan local sessions");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file, "__sessions/local.md");
}
