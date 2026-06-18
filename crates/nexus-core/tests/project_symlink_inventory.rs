use std::{fs, path::Path, sync::Arc};

use nexus_core::{
    database::Database,
    services::{paths, project_symlinks::ProjectSymlinkInventory, projects::ProjectService},
};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn display_path(path: &Path) -> String {
    paths::path_to_string(path, "path").expect("display path")
}

fn create_directory_link(source: &Path, target: &Path) {
    #[cfg(windows)]
    nexus_core::services::symlink::create_junction_placement(source, target)
        .expect("create junction link");
    #[cfg(not(windows))]
    nexus_core::services::symlink::create_symlink_placement(source, target)
        .expect("create symlink link");
}

#[test]
fn lists_and_deletes_registered_project_symlinks() {
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
                .join("target-project/shared")
        )
    );

    inventory
        .delete_project_symlink(links[0].target_path.clone())
        .expect("delete project symlink");

    assert!(inventory
        .list_project_symlinks()
        .expect("list project symlinks")
        .is_empty());
    assert!(!target_link.exists());
}
