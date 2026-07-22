use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use nexus_core::{
    database::Database,
    services::{
        outbound_request_log::OutboundRequestLogger,
        paths,
        project_symlinks::ProjectSymlinkInventory,
        projects::ProjectService,
        sync::{CreateTaskGroupInput, CreateTaskInput, SyncService},
    },
};
use serial_test::serial;
use tempfile::TempDir;

fn request_logger() -> OutboundRequestLogger {
    OutboundRequestLogger::for_test().expect("create request logger")
}

struct TestHomeGuard {
    previous_home: Option<OsString>,
    previous_userprofile: Option<OsString>,
}

impl TestHomeGuard {
    fn set(home: &Path) -> Self {
        let guard = Self {
            previous_home: env::var_os("HOME"),
            previous_userprofile: env::var_os("USERPROFILE"),
        };
        let home =
            PathBuf::from(paths::path_to_string(home, "test home").expect("display test home"));
        env::set_var("HOME", &home);
        env::set_var("USERPROFILE", &home);
        guard
    }
}

impl Drop for TestHomeGuard {
    fn drop(&mut self) {
        match self.previous_home.take() {
            Some(value) => env::set_var("HOME", value),
            None => env::remove_var("HOME"),
        }
        match self.previous_userprofile.take() {
            Some(value) => env::set_var("USERPROFILE", value),
            None => env::remove_var("USERPROFILE"),
        }
    }
}

fn set_project_symlink_ignored_dirs(db: &Database, value: &str) {
    db.connection()
        .expect("open db connection")
        .execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('project_symlink_ignored_dirs', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            [value],
        )
        .expect("set ignored dirs");
}

fn set_project_symlink_max_depth(db: &Database, value: &str) {
    db.connection()
        .expect("open db connection")
        .execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('project_symlink_max_depth', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            [value],
        )
        .expect("set max depth");
}

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

// Windows symbolic links require elevation, so tests exercise the privilege-free
// Junction action there; Unix uses Symlink. Both produce a directory link that the
// scan and task lifecycle treat uniformly.
#[cfg(windows)]
const LINK_ACTION: &str = "Junction";
#[cfg(not(windows))]
const LINK_ACTION: &str = "Symlink";

fn create_directory_link(source: &Path, target: &Path) {
    #[cfg(windows)]
    nexus_core::services::symlink::create_junction_placement(source, target)
        .expect("create junction link");
    #[cfg(not(windows))]
    nexus_core::services::symlink::create_symlink_placement(source, target)
        .expect("create symlink link");
}

fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    let path = paths::path_to_string(&path, "path").expect("display path");
    paths::collapse_home(&path)
}

fn display_path(path: &Path) -> String {
    paths::path_to_string(path, "path").expect("display path")
}

fn collapsed_display_path(path: &Path) -> String {
    paths::collapse_home(&display_path(path))
}

fn normalized_display_path(path: &Path) -> String {
    display_path(path).replace('\\', "/")
}

fn assert_link_points_to(source: &Path, target: &Path) {
    // Works for both symlinks and junctions: canonicalize resolves either to the source.
    assert_eq!(
        fs::canonicalize(target).expect("canonicalize target link"),
        fs::canonicalize(source).expect("canonicalize source")
    );
}

fn http_response(status: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

fn http_get_response(body: &str) -> String {
    let len = body.len();
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}")
}

fn http_multistatus_response(entries: &[(&str, bool, Option<u64>, Option<&str>)]) -> String {
    let mut body = String::from(r#"<?xml version="1.0"?><multistatus xmlns="DAV:">"#);
    for (href, is_collection, content_length, last_modified) in entries {
        body.push_str("<response><href>");
        body.push_str(href);
        body.push_str("</href><propstat><prop>");
        if *is_collection {
            body.push_str("<resourcetype><collection/></resourcetype>");
        }
        if let Some(len) = content_length {
            body.push_str(&format!("<getcontentlength>{len}</getcontentlength>"));
        }
        if let Some(lm) = last_modified {
            body.push_str(&format!("<getlastmodified>{lm}</getlastmodified>"));
        }
        body.push_str("</prop></propstat></response>");
    }
    body.push_str("</multistatus>");
    let len = body.len();
    format!("HTTP/1.1 207 Multi-Status\r\nContent-Type: text/xml; charset=utf-8\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}")
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
            let request_line = raw.lines().next().unwrap_or("").to_string();
            let depth = raw
                .lines()
                .find(|line| line.to_ascii_lowercase().starts_with("depth:"))
                .unwrap_or("")
                .to_string();
            requests_for_thread
                .lock()
                .expect("lock request log")
                .push(format!("{request_line} {depth}\n{raw}"));
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

#[test]
#[serial]
fn lists_session_backup_copy_task_from_project_template() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let project = projects
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    let backups = sync.list_session_backups().expect("list session backups");

    assert_eq!(backups.len(), 1);
    let backup = &backups[0];
    assert_eq!(backup.project_key, "agent-nexus");
    assert_eq!(backup.task.direction, "Push");
    assert_eq!(backup.task.action, "Copy");
    assert_eq!(backup.task.source_type, "Local");
    assert_eq!(
        backup.task.source,
        format!("{}/.sessions/", project.path.trim_end_matches('/'))
    );
    assert_eq!(backup.task.target_type, "Cloud");
    assert_eq!(backup.task.target, "Session/agent-nexus/");
    assert_eq!(backup.task.schedule, "0 * * * *");
    assert_eq!(backup.task.status, "never");
}

#[test]
#[serial]
fn session_backup_source_is_collapsed_to_tilde_for_display() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    // Canonicalize so the recorded (canonical) project path lands under $HOME.
    let home = fs::canonicalize(root.path()).expect("canonicalize home");

    let _home_guard = TestHomeGuard::set(&home);
    let result = (|| {
        projects.record_project(git_repo(&root, "agent-nexus"))?;
        sync.list_session_backups()
    })();

    let backups = result.expect("list session backups");
    assert_eq!(backups.len(), 1);
    // Local source under $HOME is displayed collapsed; the cloud target is relative.
    assert_eq!(backups[0].task.source, "~/agent-nexus/.sessions/");
    assert_eq!(backups[0].task.target, "Session/agent-nexus/");
}

#[test]
#[serial]
fn project_template_does_not_reinterpret_variable_values() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let project = projects
        .record_project(git_repo(&root, "prefix-{{project_key}}"))
        .expect("record project");

    let backups = sync.list_session_backups().expect("list session backups");

    assert_eq!(
        backups[0].task.source,
        format!("{}/.sessions/", project.path.trim_end_matches('/'))
    );
}

#[test]
#[serial]
fn session_backup_schedule_survives_template_reconciliation() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    projects
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");
    let backup = sync
        .list_session_backups()
        .expect("list session backups")
        .remove(0);

    sync.update_task_schedule(backup.task.id, "30 * * * *".to_string())
        .expect("update session backup schedule");

    let reloaded = sync.list_session_backups().expect("reload session backups");
    assert_eq!(reloaded[0].task.schedule, "30 * * * *");
}

#[tokio::test]
#[serial]
async fn automatically_pushes_due_session_backup_and_records_status() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    fs::create_dir_all(Path::new(&repo).join(".sessions")).expect("create sessions dir");
    fs::write(
        Path::new(&repo).join(".sessions").join("260623-session.md"),
        "# Session\n",
    )
    .expect("write session");
    projects.record_project(repo).expect("record project");
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");

    let ran = sync
        .run_due_scheduled_tasks(3_600)
        .await
        .expect("run due session backup");

    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].status, "ok");
    assert!(sync
        .list_task_groups()
        .expect("list custom groups")
        .is_empty());
    let backups = sync.list_session_backups().expect("list session backups");
    assert_eq!(backups[0].task.status, "ok");
    assert!(backups[0].task.last_run_at.is_some());
    assert!(sync
        .run_due_scheduled_tasks(3_620)
        .await
        .expect("repeat scheduler check")
        .is_empty());

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(requests[3].starts_with(
        "PUT /webdav/agent-nexus-sync/Session/agent-nexus/260623-session.md HTTP/1.1"
    ));
    assert!(requests[3].contains("# Session"));
}

#[tokio::test]
#[serial]
async fn skips_due_session_backup_when_local_sessions_directory_is_missing() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    projects
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    let ran = sync
        .run_due_scheduled_tasks(3_600)
        .await
        .expect("run due session backup");

    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].status, "skipped");
    let backups = sync.list_session_backups().expect("list session backups");
    assert_eq!(backups[0].task.status, "skipped");
    assert!(backups[0].task.last_run_at.is_some());
}

#[tokio::test]
#[serial]
async fn manually_running_session_backup_skips_missing_local_sessions_directory() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    projects
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");
    let backup = sync
        .list_session_backups()
        .expect("list session backups")
        .remove(0);

    let task = sync
        .run_task(backup.task.id)
        .await
        .expect("run session backup");

    assert_eq!(task.status, "skipped");
    assert!(task.last_run_at.is_some());
}

#[test]
#[serial]
fn saves_and_reads_webdav_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let saved = sync
        .save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
            url: " https://dav.example.com/root/ ".to_string(),
            user: " alice ".to_string(),
            pass: "secret".to_string(),
            remote_root: " nexus-sync ".to_string(),
        })
        .expect("save webdav settings");

    assert_eq!(saved.url, "https://dav.example.com/root/");
    assert_eq!(saved.user, "alice");
    assert_eq!(saved.pass, "secret");
    assert_eq!(saved.remote_root, "nexus-sync");
    assert_eq!(
        sync.get_webdav_settings().expect("read webdav settings"),
        saved
    );
}

#[test]
#[serial]
fn defaults_blank_webdav_remote_root() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let saved = sync
        .save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
            url: "https://dav.example.com/root/".to_string(),
            user: "alice".to_string(),
            pass: "secret".to_string(),
            remote_root: " ".to_string(),
        })
        .expect("save webdav settings");

    assert_eq!(saved.remote_root, "agent-nexus-sync");
}

#[tokio::test]
#[serial]
async fn tests_webdav_connection_and_creates_remote_root() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("207 Multi-Status"),
        http_response("201 Created"),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    sync.test_webdav_connection(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .await
    .expect("test webdav connection");

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(requests[0].starts_with("PROPFIND /webdav/ HTTP/1.1"));
    assert!(requests[0].contains("depth: 0") || requests[0].contains("Depth: 0"));
    assert!(requests[1].starts_with("MKCOL /webdav/agent-nexus-sync/ HTTP/1.1"));
}

#[tokio::test]
#[serial]
async fn runs_local_file_copy_task_to_webdav() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let source_file = root.path().join("settings.toml");
    fs::write(&source_file, "theme = 'dark'\n").expect("write source file");
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Warp".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_file.to_string_lossy().into_owned(),
                target_type: "Cloud".to_string(),
                target: "config/warp/settings.toml".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create cloud copy task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run cloud copy task");

    assert_eq!(task.status, "ok");
    assert!(task.last_run_at.is_some());
    server.join().expect("join webdav server");
    {
        let requests = requests.lock().expect("lock request log");
        assert!(requests[0].starts_with("MKCOL /webdav/agent-nexus-sync/ HTTP/1.1"));
        assert!(requests[1].starts_with("MKCOL /webdav/agent-nexus-sync/config/ HTTP/1.1"));
        assert!(requests[2].starts_with("MKCOL /webdav/agent-nexus-sync/config/warp/ HTTP/1.1"));
        assert!(requests[3]
            .starts_with("PUT /webdav/agent-nexus-sync/config/warp/settings.toml HTTP/1.1"));
        assert!(requests[3].contains("theme = 'dark'"));
    }

    let second_run = sync
        .run_due_scheduled_tasks(320)
        .await
        .expect("same minute check does not fail");
    assert!(second_run.is_empty());
}

#[tokio::test]
#[serial]
async fn runs_due_local_to_cloud_copy_task_on_schedule() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let source_file = root.path().join("settings.toml");
    fs::write(&source_file, "theme = 'dark'\n").expect("write source file");
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    sync.create_task_group(CreateTaskGroupInput {
        name: "Warp".to_string(),
        tasks: vec![CreateTaskInput {
            action: "Copy".to_string(),
            source_type: "Local".to_string(),
            source: source_file.to_string_lossy().into_owned(),
            target_type: "Cloud".to_string(),
            target: "config/warp/settings.toml".to_string(),
            schedule: "*/5 * * * *".to_string(),
        }],
    })
    .expect("create scheduled cloud copy task");

    let ran = sync
        .run_due_scheduled_tasks(300)
        .await
        .expect("run due scheduled tasks");

    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].status, "ok");
    assert!(ran[0].last_run_at.is_some());
    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(
        requests[3].starts_with("PUT /webdav/agent-nexus-sync/config/warp/settings.toml HTTP/1.1")
    );
    assert!(requests[3].contains("theme = 'dark'"));
}

#[tokio::test]
#[serial]
async fn runs_local_file_copy_task_with_tilde_source_to_webdav() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create home dir");
    let source_file = home.join("keymap.json");
    fs::write(&source_file, "keymap = '[]'\n").expect("write source file");
    let _home_guard = TestHomeGuard::set(&home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Zed".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "~/keymap.json".to_string(),
                target_type: "Cloud".to_string(),
                target: "config/zed/keymap.json".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].source, "~/keymap.json");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run tilde source task");

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert_eq!(task.status, "ok");
    assert!(requests[3].starts_with("PUT /webdav/agent-nexus-sync/config/zed/keymap.json HTTP/1.1"));
    assert!(requests[3].contains("keymap = '[]'"));
}

#[tokio::test]
#[serial]
async fn runs_local_file_copy_task_with_appdata_source_to_webdav() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let appdata = root.path().join("Roaming");
    fs::create_dir_all(appdata.join("Zed")).expect("create APPDATA dir");
    let source_file = appdata.join("Zed").join("settings.json");
    fs::write(&source_file, "theme = 'light'\n").expect("write source file");
    let previous_appdata = env::var_os("APPDATA");
    env::set_var("APPDATA", &appdata);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Zed APPDATA".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "%APPDATA%/Zed/settings.json".to_string(),
                target_type: "Cloud".to_string(),
                target: "config/zed/settings.json".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run APPDATA source task");
    match previous_appdata {
        Some(value) => env::set_var("APPDATA", value),
        None => env::remove_var("APPDATA"),
    }

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert_eq!(task.status, "ok");
    assert!(
        requests[3].starts_with("PUT /webdav/agent-nexus-sync/config/zed/settings.json HTTP/1.1")
    );
    assert!(requests[3].contains("theme = 'light'"));
}

#[tokio::test]
#[serial]
async fn runs_local_directory_copy_task_to_webdav() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("ssh");
    fs::create_dir_all(&source_dir).expect("create source dir");
    fs::write(source_dir.join("config"), "Host *\n").expect("write source file");
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "SSH".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Cloud".to_string(),
                target: "backups/ssh/".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create cloud copy task");

    sync.run_task(created.tasks[0].id.clone())
        .await
        .expect("run cloud copy task");

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(requests[0].starts_with("MKCOL /webdav/agent-nexus-sync/ HTTP/1.1"));
    assert!(requests[1].starts_with("MKCOL /webdav/agent-nexus-sync/backups/ HTTP/1.1"));
    assert!(requests[2].starts_with("MKCOL /webdav/agent-nexus-sync/backups/ssh/ HTTP/1.1"));
    assert!(requests[3].starts_with("PUT /webdav/agent-nexus-sync/backups/ssh/config HTTP/1.1"));
    assert!(requests[3].contains("Host *"));
}

#[test]
#[serial]
fn creates_symlink_placement_and_lists_custom_task_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP symlinks".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source_dir.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: target_link.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task group");

    let groups = sync.list_task_groups().expect("list task groups");

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "TAP symlinks");
    assert_eq!(groups[0].tasks.len(), 1);
    assert_eq!(groups[0].tasks[0].direction, "Distribution");
    assert_eq!(groups[0].tasks[0].action, LINK_ACTION);
    assert_eq!(
        groups[0].tasks[0].source,
        normalized_display_path(&source_dir)
    );
    assert_eq!(
        groups[0].tasks[0].target,
        normalized_display_path(&target_link)
    );
    assert_eq!(groups[0].tasks[0].schedule, "manual");
    assert!(groups[0].tasks[0].last_run_at.is_none());
    assert_eq!(groups[0].tasks[0].status, "never");
    assert_link_points_to(&source_dir, &target_link);
}

#[test]
#[serial]
fn creates_symlink_placement_with_tilde_paths() {
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create home dir");
    let source_dir = home.join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let target_link = home.join("target-link");
    let _home_guard = TestHomeGuard::set(&home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP tilde".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: "~/source".to_string(),
            target_type: "Local".to_string(),
            target: "~/target-link".to_string(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].source, "~/source");
    assert_eq!(groups[0].tasks[0].target, "~/target-link");
    assert_link_points_to(&source_dir, &target_link);
}

#[test]
#[serial]
fn lists_symlink_task_with_present_link_state_when_placement_exists() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP symlinks".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source_dir.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: target_link.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].link_state, "present");
}

#[test]
#[serial]
fn marks_symlink_task_link_state_missing_when_placement_removed_manually() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP symlinks".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source_dir.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: target_link.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task group");

    // Simulate the user deleting the symlink out-of-band.
    #[cfg(unix)]
    fs::remove_file(&target_link).expect("remove symlink manually");
    #[cfg(windows)]
    fs::remove_dir(&target_link).expect("remove junction manually");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].link_state, "missing");
    // Task record itself is unchanged — only the derived placement state flips.
    assert_eq!(groups[0].tasks[0].status, "never");
}

#[test]
#[serial]
fn derives_link_state_for_tilde_target() {
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create home dir");
    let source_dir = home.join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let link = home.join("link");
    let _home_guard = TestHomeGuard::set(&home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP tilde state".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: "~/source".to_string(),
            target_type: "Local".to_string(),
            target: "~/link".to_string(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].link_state, "present");

    #[cfg(unix)]
    fs::remove_file(&link).expect("remove symlink manually");
    #[cfg(windows)]
    fs::remove_dir(&link).expect("remove junction manually");

    let groups = sync
        .list_task_groups()
        .expect("list task groups after remove");
    assert_eq!(groups[0].tasks[0].link_state, "missing");
}

#[test]
#[serial]
fn rejects_cloud_to_cloud_task() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let error = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Cloud task".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Cloud".to_string(),
                source: "config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backup/config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect_err("cloud to cloud tasks are unsupported");

    assert!(error.to_string().contains("Cloud to Cloud"));
}

#[test]
#[serial]
fn creates_local_to_cloud_copy_task_with_scheduled_cron() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Scheduled task".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backups/config".to_string(),
                schedule: "*/5 * * * *".to_string(),
            }],
        })
        .expect("create scheduled task");

    assert_eq!(created.tasks[0].direction, "Push");
    assert_eq!(created.tasks[0].schedule, "*/5 * * * *");
    assert_eq!(
        sync.list_task_groups().expect("list task groups")[0].tasks[0].schedule,
        "*/5 * * * *"
    );
}

#[test]
#[serial]
fn updates_copy_task_schedule_and_lists_the_saved_value() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Scheduled task".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backups/config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task");

    let updated = sync
        .update_task_schedule(created.tasks[0].id.clone(), "0 * * * *".to_string())
        .expect("update task schedule");

    assert_eq!(updated.schedule, "0 * * * *");
    assert_eq!(
        sync.list_task_groups().expect("list task groups")[0].tasks[0].schedule,
        "0 * * * *"
    );
}

#[test]
#[serial]
fn group_schedule_bulk_applies_to_copy_tasks_and_re_overrides_per_task_schedule() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Mixed group".to_string(),
            tasks: vec![
                CreateTaskInput {
                    action: "Copy".to_string(),
                    source_type: "Local".to_string(),
                    source: "/workspace/a".to_string(),
                    target_type: "Cloud".to_string(),
                    target: "backups/a".to_string(),
                    schedule: "manual".to_string(),
                },
                CreateTaskInput {
                    action: "Copy".to_string(),
                    source_type: "Local".to_string(),
                    source: "/workspace/b".to_string(),
                    target_type: "Cloud".to_string(),
                    target: "backups/b".to_string(),
                    schedule: "*/5 * * * *".to_string(),
                },
                CreateTaskInput {
                    action: LINK_ACTION.to_string(),
                    source_type: "Local".to_string(),
                    source: source_dir.to_string_lossy().into_owned(),
                    target_type: "Local".to_string(),
                    target: target_link.to_string_lossy().into_owned(),
                    schedule: "manual".to_string(),
                },
            ],
        })
        .expect("create mixed group");

    // Group schedule bulk-applies to every Copy task, regardless of their prior values,
    // and leaves the non-schedulable link task untouched.
    sync.update_group_schedule(created.id.clone(), "0 * * * *".to_string())
        .expect("apply group schedule");
    let groups = sync.list_task_groups().expect("list task groups");
    for task in &groups[0].tasks {
        let expected = if task.action == "Copy" {
            "0 * * * *"
        } else {
            "manual"
        };
        assert_eq!(task.schedule, expected);
    }

    // A per-task schedule overrides the group for that one task...
    let copy_id = groups[0]
        .tasks
        .iter()
        .find(|task| task.action == "Copy")
        .expect("a copy task")
        .id
        .clone();
    let overridden = sync
        .update_task_schedule(copy_id.clone(), "30 * * * *".to_string())
        .expect("override one task");
    assert_eq!(overridden.schedule, "30 * * * *");

    // ...and re-applying the group schedule overrides that per-task value again (last write wins).
    sync.update_group_schedule(created.id.clone(), "0 5 * * *".to_string())
        .expect("re-apply group schedule");
    let groups = sync.list_task_groups().expect("list task groups");
    for task in &groups[0].tasks {
        let expected = if task.action == "Copy" {
            "0 5 * * *"
        } else {
            "manual"
        };
        assert_eq!(task.schedule, expected);
    }
}

#[test]
#[serial]
fn group_schedule_rejects_unknown_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    assert!(sync
        .update_group_schedule("missing-group".to_string(), "0 * * * *".to_string())
        .is_err());
}

#[test]
#[serial]
fn deletes_symlink_task_and_its_local_symlink_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Local symlink".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    sync.delete_task(created.tasks[0].id.clone())
        .expect("delete task");

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks.len(), 0);
    assert!(!target_link.exists());
}

#[test]
#[serial]
fn deletes_task_group_and_its_symlink_placements() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Symlinks".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    sync.delete_task_group(created.id.clone())
        .expect("delete task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert!(groups.is_empty());
    assert!(!target_link.exists());
}

#[test]
#[serial]
fn deletes_task_and_removes_tilde_target_link() {
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create home dir");
    let source_dir = home.join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let link = home.join("link");
    let _home_guard = TestHomeGuard::set(&home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "TAP tilde delete".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: "~/source".to_string(),
                target_type: "Local".to_string(),
                target: "~/link".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");
    assert!(link.exists(), "link created at expanded home path");

    sync.delete_task(created.tasks[0].id.clone())
        .expect("delete task");
    assert!(!link.exists(), "tilde target link removed by delete_task");
}

#[test]
#[serial]
fn deletes_task_group_and_removes_tilde_target_link() {
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create home dir");
    let source_dir = home.join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let link = home.join("link");
    let _home_guard = TestHomeGuard::set(&home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "TAP tilde group delete".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: "~/source".to_string(),
                target_type: "Local".to_string(),
                target: "~/link".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");
    assert!(link.exists(), "link created at expanded home path");

    sync.delete_task_group(created.id.clone())
        .expect("delete task group");
    assert!(
        !link.exists(),
        "tilde target link removed by delete_task_group"
    );
}

#[test]
#[serial]
fn deletes_task_group_with_copy_task_without_touching_source() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_file = root.path().join("source.txt");
    fs::write(&source_file, "payload").expect("write source file");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Copy group".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_file.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: root
                    .path()
                    .join("copied.txt")
                    .to_string_lossy()
                    .into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    sync.delete_task_group(created.id.clone())
        .expect("delete task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert!(groups.is_empty());
    assert!(source_file.exists());
    assert_eq!(
        fs::read_to_string(&source_file).expect("read source file"),
        "payload"
    );
}

#[test]
#[serial]
fn deletes_mixed_task_group_cleans_symlink_but_preserves_copy_source() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let link_source = root.path().join("link-source");
    let target_link = root.path().join("target-link");
    let copy_source = root.path().join("copy-source.txt");
    fs::create_dir_all(&link_source).expect("create link source dir");
    fs::write(&copy_source, "copy-payload").expect("write copy source file");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Mixed".to_string(),
            tasks: vec![
                CreateTaskInput {
                    action: LINK_ACTION.to_string(),
                    source_type: "Local".to_string(),
                    source: link_source.to_string_lossy().into_owned(),
                    target_type: "Local".to_string(),
                    target: target_link.to_string_lossy().into_owned(),
                    schedule: "manual".to_string(),
                },
                CreateTaskInput {
                    action: "Copy".to_string(),
                    source_type: "Local".to_string(),
                    source: copy_source.to_string_lossy().into_owned(),
                    target_type: "Local".to_string(),
                    target: root
                        .path()
                        .join("copied.txt")
                        .to_string_lossy()
                        .into_owned(),
                    schedule: "manual".to_string(),
                },
            ],
        })
        .expect("create task group");

    sync.delete_task_group(created.id.clone())
        .expect("delete task group");

    let groups = sync.list_task_groups().expect("list task groups");
    assert!(groups.is_empty());
    assert!(!target_link.exists());
    assert!(copy_source.exists());
    assert_eq!(
        fs::read_to_string(&copy_source).expect("read copy source"),
        "copy-payload"
    );
}

#[test]
#[serial]
fn deletes_unknown_task_group_id_is_idempotent() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    sync.delete_task_group("nonexistent-id".to_string())
        .expect("deleting unknown group is idempotent");

    let groups = sync.list_task_groups().expect("list task groups");
    assert!(groups.is_empty());
}

#[test]
#[serial]
fn task_group_collapsed_defaults_and_round_trips() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Collapsible group".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backup/config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    assert!(!created.collapsed, "new groups should be expanded");

    let collapsed = sync
        .set_task_group_collapsed(created.id.clone(), true)
        .expect("collapse task group");
    assert!(collapsed.collapsed);
    assert_eq!(collapsed.tasks, created.tasks);

    let expanded = sync
        .set_task_group_collapsed(created.id.clone(), false)
        .expect("expand task group");
    assert!(!expanded.collapsed);
    assert!(
        !sync
            .list_task_groups()
            .expect("list task groups")
            .into_iter()
            .find(|group| group.id == created.id)
            .expect("group exists")
            .collapsed
    );
}

#[test]
#[serial]
fn set_task_group_collapsed_rejects_unknown_and_system_groups() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    {
        let conn = db.connection().expect("open db connection");
        conn.execute(
            "INSERT INTO task_groups (id, name, system_kind, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            ("system:session-backup", "Session Backup", "session_backup", 0_i64),
        )
        .expect("insert system task group");
    }
    let sync = SyncService::new(db, request_logger());

    let unknown = sync
        .set_task_group_collapsed("missing".to_string(), true)
        .expect_err("unknown group should be rejected");
    assert!(unknown.to_string().contains("task group not found"));

    let system = sync
        .set_task_group_collapsed("system:session-backup".to_string(), true)
        .expect_err("system group should be rejected");
    assert!(system.to_string().contains("task group not found"));
}

#[test]
#[serial]
fn renames_task_group_successfully() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Original name".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backup/config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let renamed = sync
        .rename_task_group(created.id.clone(), "  Renamed group  ".to_string())
        .expect("rename task group");

    assert_eq!(renamed.id, created.id);
    assert_eq!(renamed.name, "Renamed group");
    assert_eq!(renamed.tasks, created.tasks);
    assert_eq!(
        sync.list_task_groups().expect("list task groups")[0].name,
        "Renamed group"
    );
}

#[test]
#[serial]
fn rename_task_group_rejects_blank_name() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Original name".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backup/config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let error = sync
        .rename_task_group(created.id, "   ".to_string())
        .expect_err("blank task group name should fail");

    assert!(error.to_string().contains("task group name is required"));
}

#[test]
#[serial]
fn rename_task_group_rejects_unknown_group_id() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let error = sync
        .rename_task_group("nonexistent-uuid".to_string(), "Renamed group".to_string())
        .expect_err("unknown task group should fail");

    assert!(error.to_string().contains("task group not found"));
}

#[test]
#[serial]
fn rename_task_group_rejects_system_group_id() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    projects
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");
    let system_group_id = "system:session-backup".to_string();

    let error = sync
        .rename_task_group(system_group_id, "Renamed group".to_string())
        .expect_err("system task group should fail");

    assert!(error.to_string().contains("task group not found"));
}

#[test]
#[serial]
fn adds_symlink_task_to_existing_group_and_creates_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let first_source = root.path().join("first-source");
    let first_link = root.path().join("first-link");
    let second_source = root.path().join("second-source");
    let second_link = root.path().join("second-link");
    fs::create_dir_all(&first_source).expect("create first source dir");
    fs::create_dir_all(&second_source).expect("create second source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Links".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: first_source.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: first_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let updated = sync
        .add_task(
            created.id.clone(),
            CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: second_source.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: second_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            },
        )
        .expect("add symlink task to group");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.tasks.len(), 2);
    assert_eq!(
        updated.tasks[1].source,
        normalized_display_path(&second_source)
    );
    assert_eq!(
        updated.tasks[1].target,
        normalized_display_path(&second_link)
    );
    assert_eq!(updated.tasks[1].action, LINK_ACTION);
    assert_link_points_to(&second_source, &second_link);
}

#[test]
#[serial]
fn adds_copy_task_appended_after_existing_tasks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let first_source = root.path().join("first-source");
    let first_link = root.path().join("first-link");
    let copy_source = root.path().join("copy-source.txt");
    fs::create_dir_all(&first_source).expect("create first source dir");
    fs::write(&copy_source, "payload").expect("write copy source");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Mixed".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: first_source.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: first_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let updated = sync
        .add_task(
            created.id.clone(),
            CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: copy_source.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: root
                    .path()
                    .join("copied.txt")
                    .to_string_lossy()
                    .into_owned(),
                schedule: "manual".to_string(),
            },
        )
        .expect("add copy task to group");

    assert_eq!(updated.tasks.len(), 2);
    assert_eq!(updated.tasks[0].action, LINK_ACTION);
    assert_eq!(updated.tasks[1].action, "Copy");
    assert_eq!(
        updated.tasks[1].source,
        normalized_display_path(&copy_source)
    );
}

#[test]
#[serial]
fn create_task_group_normalizes_saved_paths_to_forward_slashes() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source = root.path().join("source.txt");
    fs::write(&source, "payload").expect("write source file");

    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Normalized create".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: display_path(&source).replace('/', r#"\"#),
                target_type: "Cloud".to_string(),
                target: r#"config\zed\settings.json"#.to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create normalized task group");

    assert_eq!(created.tasks.len(), 1);
    assert_eq!(created.tasks[0].source, normalized_display_path(&source));
    assert_eq!(created.tasks[0].target, "config/zed/settings.json");
}

#[test]
#[serial]
fn add_task_normalizes_saved_paths_to_forward_slashes() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source = root.path().join("source.txt");
    let target = root.path().join("restore").join("settings.json");
    fs::write(&source, "payload").expect("write source file");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Normalized add".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: display_path(&source),
                target_type: "Cloud".to_string(),
                target: "backup/settings.json".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create base task group");

    let updated = sync
        .add_task(
            created.id.clone(),
            CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Cloud".to_string(),
                source: r#"config\zed\settings.json"#.to_string(),
                target_type: "Local".to_string(),
                target: display_path(&target).replace('/', r#"\"#),
                schedule: "manual".to_string(),
            },
        )
        .expect("add normalized task");

    assert_eq!(updated.tasks.len(), 2);
    assert_eq!(updated.tasks[1].source, "config/zed/settings.json");
    assert_eq!(updated.tasks[1].target, normalized_display_path(&target));
}

#[test]
#[serial]
fn rejects_add_task_to_unknown_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());

    let error = sync
        .add_task(
            "nonexistent-group".to_string(),
            CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/tmp/source".to_string(),
                target_type: "Local".to_string(),
                target: "/tmp/target".to_string(),
                schedule: "manual".to_string(),
            },
        )
        .expect_err("adding task to unknown group should fail");

    assert!(error.to_string().contains("task group not found"));
}

#[test]
#[serial]
fn rejects_add_cloud_to_cloud_task_without_creating_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Group".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");

    let error = sync
        .add_task(
            created.id.clone(),
            CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Cloud".to_string(),
                source: "config".to_string(),
                target_type: "Cloud".to_string(),
                target: "backup/config".to_string(),
                schedule: "manual".to_string(),
            },
        )
        .expect_err("cloud to cloud add task should fail");

    assert!(error.to_string().contains("Cloud to Cloud"));
    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].tasks.len(), 1);
}

#[test]
#[serial]
fn rejects_add_scheduled_link_task_without_creating_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Group".to_string(),
            tasks: vec![CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_link.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task group");
    let new_target = root.path().join("new-link");

    let error = sync
        .add_task(
            created.id.clone(),
            CreateTaskInput {
                action: LINK_ACTION.to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: new_target.to_string_lossy().into_owned(),
                schedule: "0 5 * * *".to_string(),
            },
        )
        .expect_err("scheduled link task should fail");

    assert!(error.to_string().contains("only Copy tasks"));
    assert!(!new_target.exists());
    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks.len(), 1);
}

#[test]
#[serial]
fn project_symlink_inventory_skips_symlinks_managed_by_sync_task() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone(), request_logger());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared");
    let managed_link = Path::new(&target_repo).join("shared");
    fs::create_dir_all(&source_dir).expect("create source dir");

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    sync.create_task_group(CreateTaskGroupInput {
        name: "Managed shared".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source_dir.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: managed_link.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task-managed symlink");

    // An unrelated symlink in the same target project must still appear.
    let orphan_source = root.path().join("orphan-source");
    let orphan_link = Path::new(&target_repo).join("orphan-link");
    fs::create_dir_all(&orphan_source).expect("create orphan source dir");
    create_directory_link(&orphan_source, &orphan_link);

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(
        links.len(),
        1,
        "only the orphan link should appear, got {links:?}"
    );
    assert!(
        links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("orphan-link")),
        "orphan link should still be listed, got {links:?}"
    );
    assert!(
        !links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("shared")),
        "task-managed link should be hidden from inventory, got {links:?}"
    );
}

#[test]
#[serial]
fn project_symlink_inventory_skips_task_managed_tilde_target() {
    let root = TempDir::new().expect("create temp dir");
    let _home_guard = TestHomeGuard::set(root.path());

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone(), request_logger());
    let inventory = ProjectSymlinkInventory::new(db);
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    fs::create_dir_all(Path::new(&source_repo).join("shared")).expect("create source dir");

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo)
        .expect("record target project");

    sync.create_task_group(CreateTaskGroupInput {
        name: "Managed tilde target".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: "~/source-project/shared".to_string(),
            target_type: "Local".to_string(),
            target: "~/target-project/shared".to_string(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task-managed symlink");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert!(
        links.is_empty(),
        "task-managed tilde target should be hidden from inventory, got {links:?}"
    );
}

#[test]
#[serial]
fn project_symlink_inventory_keeps_unmanaged_link_with_same_source_as_task() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone(), request_logger());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared");
    let managed_link = Path::new(&target_repo).join("shared");
    let unmanaged_same_source_link = Path::new(&target_repo).join("same-source-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    sync.create_task_group(CreateTaskGroupInput {
        name: "Managed shared".to_string(),
        tasks: vec![CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source_dir.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: managed_link.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        }],
    })
    .expect("create task-managed symlink");
    create_directory_link(&source_dir, &unmanaged_same_source_link);

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(
        links.len(),
        1,
        "only the unmanaged same-source link should appear, got {links:?}"
    );
    assert!(
        links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("same-source-link")),
        "unmanaged link with same source should still be listed, got {links:?}"
    );
    assert!(
        !links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("shared")),
        "task-managed link should be hidden from inventory, got {links:?}"
    );
}

#[test]
#[serial]
fn deletes_scanned_project_symlink_without_task_record() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared");
    let target_link = Path::new(&target_repo).join("shared");
    fs::create_dir_all(&source_dir).expect("create source dir");
    create_directory_link(&source_dir, &target_link);
    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo)
        .expect("record target project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");
    inventory
        .delete_project_symlink(links[0].target_path.clone())
        .expect("delete project symlink");

    assert!(inventory
        .list_project_symlinks()
        .expect("list project symlinks")
        .is_empty());
    assert!(!target_link.exists());
}

#[test]
#[serial]
fn lists_project_symlinks_with_registered_source_and_target_projects() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared-skill");
    let target_link = Path::new(&target_repo).join("shared-skill");
    fs::create_dir_all(&source_dir).expect("create source dir");
    create_directory_link(&source_dir, &target_link);

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].source_project_name.as_deref(),
        Some("source-project")
    );
    assert_eq!(
        links[0].target_project_name.as_deref(),
        Some("target-project")
    );
    assert_eq!(links[0].source_path, canonical_display_path(source_dir));
    assert_eq!(
        links[0].target_path,
        collapsed_display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/shared-skill")
        )
    );
    assert_eq!(links[0].status, "ok");
    #[cfg(windows)]
    assert_eq!(links[0].link_type, "Junction");
    #[cfg(not(windows))]
    assert_eq!(links[0].link_type, "Symlink");
}

#[test]
#[serial]
fn does_not_expand_children_inside_directory_symlink() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let target_repo = git_repo(&root, "target-project");
    let external_source = root.path().join("external-source");
    let nested_dir = external_source.join("nested");
    fs::create_dir_all(&nested_dir).expect("create nested source dir");
    fs::write(external_source.join("README.md"), "source content").expect("write source file");
    fs::write(nested_dir.join("child.md"), "nested content").expect("write nested file");

    let target_link = Path::new(&target_repo).join("external-source");
    create_directory_link(&external_source, &target_link);

    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].source_path,
        canonical_display_path(external_source)
    );
    assert_eq!(
        links[0].target_path,
        collapsed_display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/external-source")
        )
    );
    assert_eq!(links[0].link_kind, "directory");
}

#[test]
#[serial]
fn skips_configured_directories_when_scanning_project_symlinks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    set_project_symlink_ignored_dirs(&db, ".git\n.venv\nnode_modules\nvendor");
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared");
    fs::create_dir_all(&source_dir).expect("create source dir");
    fs::create_dir_all(Path::new(&target_repo).join("node_modules")).expect("create node_modules");
    fs::create_dir_all(Path::new(&target_repo).join(".venv")).expect("create .venv");
    fs::create_dir_all(Path::new(&target_repo).join("vendor")).expect("create vendor");
    fs::create_dir_all(Path::new(&target_repo).join("src")).expect("create src");
    create_directory_link(
        &source_dir,
        &Path::new(&target_repo).join("node_modules/shared"),
    );
    create_directory_link(&source_dir, &Path::new(&target_repo).join(".venv/shared"));
    create_directory_link(&source_dir, &Path::new(&target_repo).join("vendor/shared"));
    create_directory_link(&source_dir, &Path::new(&target_repo).join("src/shared"));

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target_path,
        collapsed_display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/src/shared")
        )
    );
}

#[test]
#[serial]
fn skips_common_build_output_dirs_by_default() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "build-output-project");
    let source_dir = root.path().join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");

    for build_dir in [
        "target",
        "dist",
        "build",
        "__pycache__",
        "node_modules",
        ".venv",
        ".git",
    ] {
        let dir = Path::new(&repo).join(build_dir);
        fs::create_dir_all(&dir).expect("create build output dir");
        create_directory_link(&source_dir, &dir.join("shared-link"));
    }

    projects.record_project(repo).expect("record project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");
    assert!(
        links.is_empty(),
        "default ignored dirs should skip common build outputs, got {links:?}"
    );
}

#[test]
#[serial]
fn respects_max_depth_setting() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    set_project_symlink_max_depth(&db, "2");
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "depth-project");
    let source_dir = root.path().join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");

    let repo_root = fs::canonicalize(&repo).expect("canonicalize repo");
    fs::create_dir_all(repo_root.join("d1")).expect("create d1");
    fs::create_dir_all(repo_root.join("d1").join("d2")).expect("create d2");
    fs::create_dir_all(repo_root.join("d1").join("d2").join("d3")).expect("create d3");
    create_directory_link(&source_dir, &repo_root.join("l1-link"));
    create_directory_link(&source_dir, &repo_root.join("d1").join("l2-link"));
    create_directory_link(
        &source_dir,
        &repo_root.join("d1").join("d2").join("l3-link"),
    );
    create_directory_link(
        &source_dir,
        &repo_root.join("d1").join("d2").join("d3").join("l4-link"),
    );

    projects.record_project(repo).expect("record project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");
    let has = |suffix: &str| {
        links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with(suffix))
    };

    assert!(
        has("l1-link"),
        "depth 1 link should be listed at max_depth 2, got {links:?}"
    );
    assert!(
        has("l2-link"),
        "depth 2 link should be listed at max_depth 2, got {links:?}"
    );
    assert!(
        !has("l3-link"),
        "depth 3 link should be skipped at max_depth 2, got {links:?}"
    );
    assert!(
        !has("l4-link"),
        "depth 4 link should be skipped at max_depth 2, got {links:?}"
    );
}

#[test]
#[serial]
fn skips_links_beyond_default_max_depth_without_setting_override() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "default-depth-project");
    let source_dir = root.path().join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");

    let repo_root = fs::canonicalize(&repo).expect("canonicalize repo");
    fs::create_dir_all(repo_root.join("d1").join("d2").join("d3")).expect("create deep dirs");
    create_directory_link(
        &source_dir,
        &repo_root.join("d1").join("d2").join("d3").join("l4-link"),
    );
    create_directory_link(&source_dir, &repo_root.join("l1-link"));

    projects.record_project(repo).expect("record project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");
    assert!(
        links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("l1-link")),
        "shallow link should be listed under default max_depth, got {links:?}"
    );
    assert!(
        !links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("l4-link")),
        "depth 4 link should be skipped under default max_depth 3, got {links:?}"
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn continues_scan_when_subdirectory_is_inaccessible() {
    use std::os::unix::fs::PermissionsExt;

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "acl-project");
    let source_dir = root.path().join("source");
    fs::create_dir_all(&source_dir).expect("create source dir");

    let repo_root = fs::canonicalize(&repo).expect("canonicalize repo");
    let private_dir = repo_root.join("private");
    fs::create_dir_all(&private_dir).expect("create private dir");
    fs::write(private_dir.join("secret.txt"), "x").expect("write secret file");
    create_directory_link(&source_dir, &repo_root.join("shared-link"));

    fs::set_permissions(&private_dir, fs::Permissions::from_mode(0o000))
        .expect("remove directory permissions");

    projects.record_project(repo).expect("record project");

    let result = inventory.list_project_symlinks();

    fs::set_permissions(&private_dir, fs::Permissions::from_mode(0o700))
        .expect("restore directory permissions");

    let links = result.expect("scan should not fail on inaccessible subdirectory");
    assert!(
        links
            .iter()
            .any(|l| Path::new(&l.target_path).ends_with("shared-link")),
        "accessible symlink should still be listed when a sibling directory is inaccessible, got {links:?}"
    );
}

#[tokio::test]
#[serial]
async fn runs_local_to_local_file_copy_to_nonexistent_target() {
    let root = TempDir::new().expect("create temp dir");
    let source_file = root.path().join("source.txt");
    fs::write(&source_file, "hello").expect("write source file");
    let target_file = root.path().join("target.txt");

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Local copy".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_file.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_file.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create local copy task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run local copy task");

    assert_eq!(task.status, "ok");
    assert_eq!(
        fs::read_to_string(&target_file).expect("read copied target"),
        "hello"
    );
}

#[tokio::test]
#[serial]
async fn runs_local_to_local_directory_copy_to_nonexistent_target() {
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    fs::create_dir_all(source_dir.join("sub")).expect("create nested dirs");
    fs::write(source_dir.join("a.txt"), "alpha").expect("write a.txt");
    fs::write(source_dir.join("sub").join("b.txt"), "beta").expect("write sub/b.txt");
    let target_dir = root.path().join("target");

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Local dir copy".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_dir.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create local dir copy task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run local dir copy task");

    assert_eq!(task.status, "ok");
    assert!(target_dir.is_dir());
    assert_eq!(
        fs::read_to_string(target_dir.join("a.txt")).expect("read copied a.txt"),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(target_dir.join("sub").join("b.txt")).expect("read copied sub/b.txt"),
        "beta"
    );
}

#[tokio::test]
#[serial]
async fn runs_local_to_local_directory_copy_embeds_into_existing_directory_target() {
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    fs::create_dir_all(source_dir.join("sub")).expect("create nested dirs");
    fs::write(source_dir.join("a.txt"), "alpha").expect("write a.txt");
    fs::write(source_dir.join("sub").join("b.txt"), "beta").expect("write sub/b.txt");

    let target_dir = root.path().join("target");
    fs::create_dir_all(&target_dir).expect("create existing target dir");
    fs::write(target_dir.join("c.txt"), "gamma").expect("write pre-existing sibling");
    let embedded = target_dir.join("source");
    fs::create_dir_all(&embedded).expect("create pre-existing embedded dir");
    fs::write(embedded.join("stale.txt"), "stale").expect("write stale embedded file");

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Local dir copy".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: source_dir.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_dir.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create local dir copy task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run local dir copy task");

    assert_eq!(task.status, "ok");
    // cp -r 嵌入式语义：源目录嵌入为 target/source/...
    assert_eq!(
        fs::read_to_string(target_dir.join("source").join("a.txt")).expect("read embedded a.txt"),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(target_dir.join("source").join("sub").join("b.txt"))
            .expect("read embedded sub/b.txt"),
        "beta"
    );
    // 已有 sibling 不受影响
    assert_eq!(
        fs::read_to_string(target_dir.join("c.txt")).expect("read c.txt unchanged"),
        "gamma"
    );
    // 增量复制不删除目标中已有但源中不存在的文件
    assert!(
        target_dir.join("source").join("stale.txt").exists(),
        "stale embedded file should be preserved under incremental copy"
    );
}

#[tokio::test]
#[serial]
async fn runs_local_to_local_copy_rejects_missing_source_with_validation() {
    let root = TempDir::new().expect("create temp dir");
    let missing_source = root.path().join("nope.txt");
    let target_file = root.path().join("target.txt");

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Missing source".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: missing_source.to_string_lossy().into_owned(),
                target_type: "Local".to_string(),
                target: target_file.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create task with missing source");

    let err = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect_err("missing source should fail");

    match err {
        nexus_core::error::AppError::Validation(msg) => assert!(
            msg.contains("source does not exist"),
            "unexpected validation message: {msg}"
        ),
        other => panic!("expected Validation, got {other:?}"),
    }
    assert!(
        !target_file.exists(),
        "target must not be created on failure"
    );
}

#[tokio::test]
#[serial]
async fn session_backup_skips_unchanged_files_and_pushes_only_changed() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
        http_response("201 Created"),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    let sessions_dir = Path::new(&repo).join(".sessions");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::write(sessions_dir.join("a.md"), "# A\n").expect("write a");
    fs::write(sessions_dir.join("b.md"), "# B\n").expect("write b");
    projects.record_project(repo).expect("record project");
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");

    let backup = sync
        .list_session_backups()
        .expect("list session backups")
        .remove(0);

    let task = sync
        .run_task(backup.task.id.clone())
        .await
        .expect("first run");
    assert_eq!(task.status, "ok");
    {
        let reqs = requests.lock().expect("lock request log");
        let put_count = reqs.iter().filter(|r| r.starts_with("PUT ")).count();
        assert_eq!(put_count, 2, "first run should push both files");
    }

    let task2 = sync
        .run_task(backup.task.id.clone())
        .await
        .expect("second run");
    assert_eq!(task2.status, "ok");
    {
        let reqs = requests.lock().expect("lock request log");
        let put_count = reqs.iter().filter(|r| r.starts_with("PUT ")).count();
        assert_eq!(put_count, 2, "second run should push zero new files");
    }

    fs::write(sessions_dir.join("a.md"), "# A modified\n").expect("modify a");

    let task3 = sync.run_task(backup.task.id).await.expect("third run");
    assert_eq!(task3.status, "ok");
    {
        let reqs = requests.lock().expect("lock request log");
        let put_count = reqs.iter().filter(|r| r.starts_with("PUT ")).count();
        assert_eq!(put_count, 3, "third run should push only the changed file");
    }

    server.join().expect("join webdav server");
}

#[tokio::test]
#[serial]
async fn runs_cloud_to_local_file_pull_task() {
    let (url, _requests, server) = spawn_webdav_server(vec![http_get_response("theme = 'dark'\n")]);
    let root = TempDir::new().expect("create temp dir");
    let target_file = root.path().join("settings.toml");
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Restore".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Cloud".to_string(),
                source: "config/warp/settings.toml".to_string(),
                target_type: "Local".to_string(),
                target: target_file.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create cloud pull task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run cloud pull task");

    assert_eq!(task.status, "ok");
    assert!(target_file.exists(), "target file should be created");
    assert_eq!(
        fs::read_to_string(&target_file).expect("read pulled target"),
        "theme = 'dark'\n"
    );
    server.join().expect("join webdav server");
}

#[tokio::test]
#[serial]
async fn runs_cloud_to_local_directory_pull_task() {
    let (url, _requests, server) = spawn_webdav_server(vec![
        http_multistatus_response(&[
            ("/webdav/agent-nexus-sync/config/warp/", true, None, None),
            (
                "/webdav/agent-nexus-sync/config/warp/a.txt",
                false,
                Some(5),
                Some("Sun, 06 Nov 1994 08:49:37 GMT"),
            ),
            (
                "/webdav/agent-nexus-sync/config/warp/b.txt",
                false,
                Some(4),
                Some("Sun, 06 Nov 1994 08:49:38 GMT"),
            ),
        ]),
        http_get_response("alpha"),
        http_get_response("beta"),
    ]);
    let root = TempDir::new().expect("create temp dir");
    let target_dir = root.path().join("warp");
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    sync.save_webdav_settings(nexus_core::services::sync::WebdavSettingsInput {
        url,
        user: "alice".to_string(),
        pass: "secret".to_string(),
        remote_root: "agent-nexus-sync".to_string(),
    })
    .expect("save webdav settings");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Restore Dir".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Cloud".to_string(),
                source: "config/warp/".to_string(),
                target_type: "Local".to_string(),
                target: target_dir.to_string_lossy().into_owned(),
                schedule: "manual".to_string(),
            }],
        })
        .expect("create cloud pull dir task");

    let task = sync
        .run_task(created.tasks[0].id.clone())
        .await
        .expect("run cloud pull dir task");

    assert_eq!(task.status, "ok");
    assert!(target_dir.is_dir(), "target dir should be created");
    assert_eq!(
        fs::read_to_string(target_dir.join("a.txt")).expect("read pulled a.txt"),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(target_dir.join("b.txt")).expect("read pulled b.txt"),
        "beta"
    );
    server.join().expect("join webdav server");
}

/// Create a local link task group with one task per `(source, target)` pair. Returns the group id.
fn create_link_group(sync: &SyncService, name: &str, links: &[(&Path, &Path)]) -> String {
    let tasks = links
        .iter()
        .map(|(source, target)| CreateTaskInput {
            action: LINK_ACTION.to_string(),
            source_type: "Local".to_string(),
            source: source.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: target.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
        })
        .collect();
    sync.create_task_group(CreateTaskGroupInput {
        name: name.to_string(),
        tasks,
    })
    .expect("create link task group")
    .id
}

#[test]
#[serial]
fn reorder_task_groups_persists_new_order() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");

    let make = |name: &str| {
        let source = root.path().join(format!("{name}-src"));
        fs::create_dir_all(&source).expect("create source dir");
        let target = root.path().join(format!("{name}-link"));
        create_link_group(&sync, name, &[(source.as_path(), target.as_path())])
    };
    let alpha = make("alpha");
    let beta = make("beta");
    let gamma = make("gamma");

    sync.reorder_task_groups(vec![gamma.clone(), alpha.clone(), beta.clone()])
        .expect("reorder task groups");

    let order: Vec<String> = sync
        .list_task_groups()
        .expect("list task groups")
        .into_iter()
        .map(|group| group.id)
        .collect();
    assert_eq!(order, vec![gamma, alpha, beta]);
}

#[test]
#[serial]
fn reorder_task_groups_rejects_incomplete_order() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");
    let source = root.path().join("src");
    fs::create_dir_all(&source).expect("create source dir");
    let target = root.path().join("link");
    let only = create_link_group(&sync, "only", &[(source.as_path(), target.as_path())]);

    let result = sync.reorder_task_groups(vec![only, "missing".to_string()]);
    assert!(result.is_err(), "order with unknown ids must be rejected");
}

#[test]
#[serial]
fn reorder_tasks_persists_new_order_within_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db, request_logger());
    let root = TempDir::new().expect("create temp dir");

    let link = |name: &str| {
        let source = root.path().join(format!("{name}-src"));
        fs::create_dir_all(&source).expect("create source dir");
        (source, root.path().join(format!("{name}-link")))
    };
    let (a_src, a_dst) = link("a");
    let (b_src, b_dst) = link("b");
    let (c_src, c_dst) = link("c");
    let group_id = create_link_group(
        &sync,
        "group",
        &[
            (a_src.as_path(), a_dst.as_path()),
            (b_src.as_path(), b_dst.as_path()),
            (c_src.as_path(), c_dst.as_path()),
        ],
    );

    let task_ids: Vec<String> = sync
        .list_task_groups()
        .expect("list task groups")
        .into_iter()
        .find(|group| group.id == group_id)
        .expect("group exists")
        .tasks
        .into_iter()
        .map(|task| task.id)
        .collect();

    let reversed: Vec<String> = task_ids.iter().rev().cloned().collect();
    let updated = sync
        .reorder_tasks(group_id.clone(), reversed.clone())
        .expect("reorder tasks");
    let updated_order: Vec<String> = updated.tasks.iter().map(|task| task.id.clone()).collect();
    assert_eq!(updated_order, reversed);

    let persisted: Vec<String> = sync
        .list_task_groups()
        .expect("list task groups")
        .into_iter()
        .find(|group| group.id == group_id)
        .expect("group exists")
        .tasks
        .into_iter()
        .map(|task| task.id)
        .collect();
    assert_eq!(persisted, reversed);
}
