use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use agent_nexus_lib::{
    database::Database,
    services::{
        paths,
        projects::ProjectService,
        symlink::create_symlink_placement,
        sync::{CreateTaskGroupInput, CreateTaskInput, SyncService},
    },
};
use tempfile::TempDir;

fn set_project_symlink_ignored_dirs(db: &Database, value: &str) {
    db.connection()
        .expect("open db connection")
        .execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('sync_project_symlink_ignored_dirs', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            [value],
        )
        .expect("set ignored dirs");
}

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn create_directory_link(source: &Path, target: &Path) {
    create_symlink_placement(source, target).expect("create directory link");
}

fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    paths::path_to_string(&path, "path").expect("display path")
}

fn display_path(path: &Path) -> String {
    paths::path_to_string(path, "path").expect("display path")
}

fn assert_link_points_to(source: &Path, target: &Path) {
    let metadata = fs::symlink_metadata(target).expect("read target link metadata");
    assert!(metadata.file_type().is_symlink());
    let raw_link = fs::read_link(target).expect("read target link");
    let resolved = if raw_link.is_absolute() {
        raw_link
    } else {
        target
            .parent()
            .map(|parent| parent.join(&raw_link))
            .unwrap_or(raw_link)
    };
    assert_eq!(
        fs::canonicalize(resolved).expect("canonicalize resolved link"),
        fs::canonicalize(source).expect("canonicalize source")
    );
}

fn http_response(status: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
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
fn saves_and_reads_webdav_settings() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    let saved = sync
        .save_webdav_settings(agent_nexus_lib::services::sync::WebdavSettingsInput {
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
fn defaults_blank_webdav_remote_root() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    let saved = sync
        .save_webdav_settings(agent_nexus_lib::services::sync::WebdavSettingsInput {
            url: "https://dav.example.com/root/".to_string(),
            user: "alice".to_string(),
            pass: "secret".to_string(),
            remote_root: " ".to_string(),
        })
        .expect("save webdav settings");

    assert_eq!(saved.remote_root, "agent-nexus-sync");
}

#[tokio::test]
async fn tests_webdav_connection_and_creates_remote_root() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("207 Multi-Status"),
        http_response("201 Created"),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    sync.test_webdav_connection(agent_nexus_lib::services::sync::WebdavSettingsInput {
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
    let sync = SyncService::new(db);
    sync.save_webdav_settings(agent_nexus_lib::services::sync::WebdavSettingsInput {
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
    assert_ne!(task.last_run, "—");
    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert!(requests[0].starts_with("MKCOL /webdav/agent-nexus-sync/ HTTP/1.1"));
    assert!(requests[1].starts_with("MKCOL /webdav/agent-nexus-sync/config/ HTTP/1.1"));
    assert!(requests[2].starts_with("MKCOL /webdav/agent-nexus-sync/config/warp/ HTTP/1.1"));
    assert!(
        requests[3].starts_with("PUT /webdav/agent-nexus-sync/config/warp/settings.toml HTTP/1.1")
    );
    assert!(requests[3].contains("theme = 'dark'"));
}

#[tokio::test]
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
    let sync = SyncService::new(db);
    sync.save_webdav_settings(agent_nexus_lib::services::sync::WebdavSettingsInput {
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
fn creates_symlink_placement_and_lists_custom_task_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");

    sync.create_task_group(CreateTaskGroupInput {
        name: "TAP symlinks".to_string(),
        tasks: vec![CreateTaskInput {
            action: "Symlink".to_string(),
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
    assert_eq!(groups[0].tasks[0].action, "Symlink");
    assert_eq!(groups[0].tasks[0].source, source_dir.to_string_lossy());
    assert_eq!(groups[0].tasks[0].target, target_link.to_string_lossy());
    assert_eq!(groups[0].tasks[0].schedule, "manual");
    assert_eq!(groups[0].tasks[0].last_run, "—");
    assert_eq!(groups[0].tasks[0].status, "never");
    assert_link_points_to(&source_dir, &target_link);
}

#[test]
fn rejects_cloud_to_cloud_task() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

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
fn rejects_scheduled_task_until_scheduler_is_implemented() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    let error = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Scheduled task".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Local".to_string(),
                target: "/workspace/target".to_string(),
                schedule: "0 5 * * *".to_string(),
            }],
        })
        .expect_err("scheduled tasks are not implemented");

    assert!(error.to_string().contains("scheduled sync tasks"));
}

#[test]
fn deletes_symlink_task_and_its_local_symlink_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_dir = root.path().join("source");
    let target_link = root.path().join("target-link");
    fs::create_dir_all(&source_dir).expect("create source dir");
    let created = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Local symlink".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Symlink".to_string(),
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
fn deletes_scanned_project_symlink_without_task_record() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db);
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

    let links = sync.list_project_symlinks().expect("list project symlinks");
    sync.delete_project_symlink(links[0].target_path.clone())
        .expect("delete project symlink");

    assert!(sync
        .list_project_symlinks()
        .expect("list project symlinks")
        .is_empty());
    assert!(!target_link.exists());
}

#[test]
fn lists_project_symlinks_with_registered_source_and_target_projects() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db);
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

    let links = sync.list_project_symlinks().expect("list project symlinks");

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
        display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/shared-skill")
        )
    );
    assert_eq!(links[0].status, "ok");
}

#[test]
fn does_not_expand_children_inside_directory_symlink() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db);
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

    let links = sync.list_project_symlinks().expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].source_path,
        canonical_display_path(external_source)
    );
    assert_eq!(
        links[0].target_path,
        display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/external-source")
        )
    );
    assert_eq!(links[0].link_kind, "directory");
}

#[test]
fn skips_configured_directories_when_scanning_project_symlinks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    set_project_symlink_ignored_dirs(&db, ".git\n.venv\nnode_modules\nvendor");
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db);
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

    let links = sync.list_project_symlinks().expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target_path,
        display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/src/shared")
        )
    );
}
