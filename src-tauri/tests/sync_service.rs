use std::{fs, path::Path, sync::Arc};

use agent_nexus_lib::{
    database::Database,
    services::{projects::ProjectService, sync::SyncService},
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
