use std::{fs, path::Path};

use nexus_core::{
    database::Database,
    services::{paths, projects::ProjectService},
};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    paths::path_to_string(&path, "path").expect("display path")
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
    assert_eq!(projects[0].path, canonical_display_path(repo));
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
    assert_eq!(projects[0].path, canonical_display_path(new_repo));
}

#[test]
fn records_git_base_folder_and_lists_canonical_path() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");

    let recorded = service
        .record_git_base_folder(root.path().to_string_lossy().into_owned())
        .expect("record git base folder");
    let folders = service
        .list_git_base_folders()
        .expect("list git base folders");

    assert_eq!(recorded.path, canonical_display_path(root.path()));
    #[cfg(windows)]
    assert!(
        !recorded.path.starts_with(r"\\?\"),
        "recorded base folder should not expose a Windows verbatim path: {}",
        recorded.path
    );
    assert_eq!(folders, vec![recorded]);
}

#[test]
fn scans_registered_base_folders_and_marks_recorded_projects() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let existing_repo = git_repo(&root, "existing");
    let new_repo = git_repo(&root, "new");
    fs::create_dir_all(root.path().join("notes")).expect("create non-git directory");

    service
        .record_project(existing_repo.clone())
        .expect("record existing project");
    service
        .record_git_base_folder(root.path().to_string_lossy().into_owned())
        .expect("record git base folder");

    let scan = service
        .scan_git_base_folders()
        .expect("scan git base folders");

    assert_eq!(scan.len(), 2);
    assert_eq!(scan[0].key, "existing");
    assert_eq!(scan[0].path, canonical_display_path(existing_repo));
    assert_eq!(scan[0].state, "recorded");
    assert_eq!(scan[1].key, "new");
    assert_eq!(scan[1].path, canonical_display_path(new_repo));
    assert_eq!(scan[1].state, "new");
}

#[test]
fn removes_git_base_folder_without_deleting_projects() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "kept-project");

    service.record_project(repo).expect("record project");
    let folder = service
        .record_git_base_folder(root.path().to_string_lossy().into_owned())
        .expect("record git base folder");

    service
        .remove_git_base_folder(folder.id)
        .expect("remove git base folder");

    assert!(service
        .list_git_base_folders()
        .expect("list git base folders")
        .is_empty());
    assert_eq!(service.list_projects().expect("list projects").len(), 1);
}
