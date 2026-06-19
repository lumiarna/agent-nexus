use std::{
    env,
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
        paths,
        project_symlinks::ProjectSymlinkInventory,
        projects::ProjectService,
        sync::{CreateTaskGroupInput, CreateTaskInput, SyncService},
    },
};
use tempfile::TempDir;
use serial_test::serial;

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

fn set_project_symlink_max_depth(db: &Database, value: &str) {
    db.connection()
        .expect("open db connection")
        .execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('sync_project_symlink_max_depth', ?1)
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
    paths::path_to_string(&path, "path").expect("display path")
}

fn display_path(path: &Path) -> String {
    paths::path_to_string(path, "path").expect("display path")
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
fn defaults_blank_webdav_remote_root() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

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
async fn tests_webdav_connection_and_creates_remote_root() {
    let (url, requests, server) = spawn_webdav_server(vec![
        http_response("207 Multi-Status"),
        http_response("201 Created"),
    ]);
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

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
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", &home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    server.join().expect("join webdav server");
    let requests = requests.lock().expect("lock request log");
    assert_eq!(task.status, "ok");
    assert!(requests[3].starts_with("PUT /webdav/agent-nexus-sync/config/zed/keymap.json HTTP/1.1"));
    assert!(requests[3].contains("keymap = '[]'"));
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
    assert_eq!(groups[0].tasks[0].source, source_dir.to_string_lossy());
    assert_eq!(groups[0].tasks[0].target, target_link.to_string_lossy());
    assert_eq!(groups[0].tasks[0].schedule, "manual");
    assert_eq!(groups[0].tasks[0].last_run, "—");
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
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", &home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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

    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks[0].source, "~/source");
    assert_eq!(groups[0].tasks[0].target, "~/target-link");
    assert_link_points_to(&source_dir, &target_link);
}

#[test]
fn lists_symlink_task_with_present_link_state_when_placement_exists() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
fn marks_symlink_task_link_state_missing_when_placement_removed_manually() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", &home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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

    let groups = sync.list_task_groups().expect("list task groups after remove");
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
    assert_eq!(groups[0].tasks[0].link_state, "missing");
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
fn deletes_task_group_and_its_symlink_placements() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", &home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
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
    let previous_home = env::var_os("HOME");
    env::set_var("HOME", &home);

    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
    assert!(!link.exists(), "tilde target link removed by delete_task_group");
}

#[test]
fn deletes_task_group_with_copy_task_without_touching_source() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
fn deletes_mixed_task_group_cleans_symlink_but_preserves_copy_source() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
fn deletes_unknown_task_group_id_is_idempotent() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    sync.delete_task_group("nonexistent-id".to_string())
        .expect("deleting unknown group is idempotent");

    let groups = sync.list_task_groups().expect("list task groups");
    assert!(groups.is_empty());
}

#[test]
fn adds_symlink_task_to_existing_group_and_creates_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    assert_eq!(updated.tasks[1].source, second_source.to_string_lossy());
    assert_eq!(updated.tasks[1].target, second_link.to_string_lossy());
    assert_eq!(updated.tasks[1].action, LINK_ACTION);
    assert_link_points_to(&second_source, &second_link);
}

#[test]
fn adds_copy_task_appended_after_existing_tasks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
    assert_eq!(updated.tasks[1].source, copy_source.to_string_lossy());
}

#[test]
fn rejects_add_task_to_unknown_group() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

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
fn rejects_add_cloud_to_cloud_task_without_creating_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
fn rejects_add_scheduled_task_without_creating_placement() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);
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
        .expect_err("scheduled add task should fail");

    assert!(error.to_string().contains("scheduled sync tasks"));
    assert!(!new_target.exists());
    let groups = sync.list_task_groups().expect("list task groups");
    assert_eq!(groups[0].tasks.len(), 1);
}

#[test]
fn project_symlink_inventory_skips_symlinks_managed_by_sync_task() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let sync = SyncService::new(db.clone());
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
        display_path(
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
        display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/src/shared")
        )
    );
}

#[test]
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
fn respects_max_depth_setting() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    set_project_symlink_max_depth(&db, "3");
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
        "depth 1 link should be listed at max_depth 3, got {links:?}"
    );
    assert!(
        has("l2-link"),
        "depth 2 link should be listed at max_depth 3, got {links:?}"
    );
    assert!(
        has("l3-link"),
        "depth 3 link should be listed at max_depth 3, got {links:?}"
    );
    assert!(
        !has("l4-link"),
        "depth 4 link should be skipped at max_depth 3, got {links:?}"
    );
}

#[test]
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
