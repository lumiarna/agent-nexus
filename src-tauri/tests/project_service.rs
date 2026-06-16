use std::fs;

use agent_nexus_lib::{database::Database, services::projects::ProjectService};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

#[test]
fn records_git_project_and_lists_it_by_folder_key() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");

    let recorded = service
        .record_project(repo.clone())
        .expect("record project");
    let projects = service.list_projects().expect("list projects");

    assert_eq!(recorded.name, "agent-nexus");
    assert_eq!(recorded.key, "agent-nexus");
    assert_eq!(recorded.status, "active");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, recorded.id);
    assert_eq!(
        projects[0].path,
        fs::canonicalize(repo).unwrap().to_string_lossy()
    );
}

#[test]
fn recording_same_project_key_updates_path_without_duplicate() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let old_root = TempDir::new().expect("create old temp dir");
    let new_root = TempDir::new().expect("create new temp dir");
    let old_repo = git_repo(&old_root, "shared-key");
    let new_repo = git_repo(&new_root, "shared-key");

    let first = service.record_project(old_repo).expect("record first path");
    let second = service
        .record_project(new_repo.clone())
        .expect("record moved path");
    let projects = service.list_projects().expect("list projects");

    assert_eq!(second.id, first.id);
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].path,
        fs::canonicalize(new_repo).unwrap().to_string_lossy()
    );
}
