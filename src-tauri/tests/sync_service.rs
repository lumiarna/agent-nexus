use std::{fs, path::Path, sync::Arc};

use agent_nexus_lib::{
    database::Database,
    services::{
        projects::ProjectService,
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

#[cfg(unix)]
fn symlink_dir(source: &Path, target: &Path) {
    std::os::unix::fs::symlink(source, target).expect("create directory symlink");
}

#[cfg(windows)]
fn symlink_dir(source: &Path, target: &Path) {
    std::os::windows::fs::symlink_dir(source, target).expect("create directory symlink");
}

fn assert_symlink_points_to(source: &Path, target: &Path) {
    let metadata = fs::symlink_metadata(target).expect("read target link metadata");
    assert!(metadata.file_type().is_symlink());
    assert_eq!(fs::read_link(target).expect("read target link"), source);
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
    assert_symlink_points_to(&source_dir, &target_link);
}

#[test]
fn rejects_cloud_task_until_cloud_sync_is_implemented() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let sync = SyncService::new(db);

    let error = sync
        .create_task_group(CreateTaskGroupInput {
            name: "Cloud task".to_string(),
            tasks: vec![CreateTaskInput {
                action: "Copy".to_string(),
                source_type: "Local".to_string(),
                source: "/workspace/config".to_string(),
                target_type: "Cloud".to_string(),
                target: "config".to_string(),
                schedule: "manual".to_string(),
            }],
        })
        .expect_err("cloud tasks are not implemented");

    assert!(error.to_string().contains("cloud sync tasks"));
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
    symlink_dir(&source_dir, &target_link);
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
    symlink_dir(&source_dir, &target_link);

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
    assert_eq!(
        links[0].source_path,
        fs::canonicalize(source_dir).unwrap().to_string_lossy()
    );
    assert_eq!(
        links[0].target_path,
        fs::canonicalize(Path::new(&target_repo).parent().unwrap())
            .unwrap()
            .join("target-project/shared-skill")
            .to_string_lossy()
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
    symlink_dir(&external_source, &target_link);

    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    let links = sync.list_project_symlinks().expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].source_path,
        fs::canonicalize(external_source).unwrap().to_string_lossy()
    );
    assert_eq!(
        links[0].target_path,
        fs::canonicalize(Path::new(&target_repo).parent().unwrap())
            .unwrap()
            .join("target-project/external-source")
            .to_string_lossy()
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
    symlink_dir(
        &source_dir,
        &Path::new(&target_repo).join("node_modules/shared"),
    );
    symlink_dir(&source_dir, &Path::new(&target_repo).join(".venv/shared"));
    symlink_dir(&source_dir, &Path::new(&target_repo).join("vendor/shared"));
    symlink_dir(&source_dir, &Path::new(&target_repo).join("src/shared"));

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
        fs::canonicalize(Path::new(&target_repo).parent().unwrap())
            .unwrap()
            .join("target-project/src/shared")
            .to_string_lossy()
    );
}
