use std::{fs, path::Path, sync::Arc};

use nexus_core::{
    database::Database,
    services::{paths, projects::ProjectService},
};
use rusqlite::params;
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
fn defaults_custom_skills_dirs_to_skills_and_replaces_with_dedup() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    let recorded = service.record_project(repo).expect("record project");

    // A freshly recorded project scans `skills` by default.
    assert_eq!(recorded.custom_skills_dirs, vec!["skills".to_string()]);

    let updated = service
        .set_project_custom_skills_dirs(
            recorded.id.clone(),
            vec![
                "skills".to_string(),
                "  .nexus/skills  ".to_string(),
                "./.nexus/skills/".to_string(),
            ],
        )
        .expect("set custom skills dirs");

    // Trimmed, and the two `.nexus/skills` spellings collapse to one entry.
    assert_eq!(
        updated.custom_skills_dirs,
        vec!["skills".to_string(), ".nexus/skills".to_string()]
    );
}

#[test]
fn rejects_custom_skills_dir_that_is_a_fixed_agent_dir() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "agent-nexus");
    let recorded = service.record_project(repo).expect("record project");

    let error = service
        .set_project_custom_skills_dirs(recorded.id, vec![".claude/skills".to_string()])
        .expect_err("agent dir must be rejected");
    assert!(error.to_string().contains("fixed agent skills dir"));
}

#[test]
fn defaults_extra_prompt_files_to_empty_and_accepts_matching_globs() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    // A freshly recorded project registers no extra prompt files.
    assert!(recorded.extra_prompt_files.is_empty());

    let updated = service
        .set_project_extra_prompt_files(
            recorded.id.clone(),
            vec![
                "AGENTS.local.md".to_string(),
                "  docs/CLAUDE.md  ".to_string(),
                "./docs/CLAUDE.md".to_string(),
            ],
        )
        .expect("set extra prompt files");

    // Trimmed, and the two `docs/CLAUDE.md` spellings collapse to one entry.
    assert_eq!(
        updated.extra_prompt_files,
        vec!["AGENTS.local.md".to_string(), "docs/CLAUDE.md".to_string()]
    );
}

#[test]
fn rejects_extra_prompt_file_that_does_not_match_a_prompt_glob() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    let error = service
        .set_project_extra_prompt_files(recorded.id, vec!["README.md".to_string()])
        .expect_err("non-matching file must be rejected");
    assert!(
        error.to_string().contains("does not match a prompt glob"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_extra_prompt_file_that_collides_with_a_primary_prompt() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    let error = service
        .set_project_extra_prompt_files(recorded.id, vec!["CLAUDE.md".to_string()])
        .expect_err("primary prompt file must be rejected");
    assert!(
        error
            .to_string()
            .contains("auto-discovered primary prompt file"),
        "unexpected error: {error}"
    );
}

#[test]
fn sets_sessions_dir_and_restores_default_when_cleared() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "agent-nexus"))
        .expect("record project");

    // A freshly recorded project uses the default session directory.
    assert_eq!(recorded.sessions_dir, "__sessions");

    let overridden = service
        .set_project_sessions_dir(recorded.id.clone(), "  .sessions/  ".to_string())
        .expect("override sessions dir");
    assert_eq!(overridden.sessions_dir, ".sessions");

    let restored = service
        .set_project_sessions_dir(recorded.id, String::new())
        .expect("restore default sessions dir");
    assert_eq!(restored.sessions_dir, "__sessions");
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
fn re_recording_moved_project_restores_active_status() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let old_root = TempDir::new().expect("create old temp dir");
    let old_repo = git_repo(&old_root, "relocated");
    let first = service.record_project(old_repo).expect("record old path");

    drop(old_root);

    let stale = service.list_projects().expect("list projects");
    assert_eq!(
        stale.iter().find(|p| p.id == first.id).unwrap().status,
        "stale"
    );

    let new_root = TempDir::new().expect("create new temp dir");
    let new_repo = git_repo(&new_root, "relocated");
    let re_recorded = service
        .record_project(new_repo)
        .expect("re-record new path");

    assert_eq!(re_recorded.id, first.id);
    assert_eq!(re_recorded.status, "active");

    let projects = service.list_projects().expect("list projects");
    let restored = projects.iter().find(|p| p.id == first.id).unwrap();
    assert_eq!(restored.status, "active");
}

#[test]
fn reorders_projects_and_persists_display_order() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let alpha = service
        .record_project(git_repo(&root, "alpha"))
        .expect("record alpha");
    let beta = service
        .record_project(git_repo(&root, "beta"))
        .expect("record beta");
    let gamma = service
        .record_project(git_repo(&root, "gamma"))
        .expect("record gamma");

    let reordered = service
        .reorder_projects(vec![gamma.id.clone(), alpha.id.clone(), beta.id.clone()])
        .expect("reorder projects");
    let relisted = service.list_projects().expect("list projects");

    assert_eq!(
        reordered.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        vec![gamma.id.as_str(), alpha.id.as_str(), beta.id.as_str()]
    );
    assert_eq!(
        relisted.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        vec![gamma.id.as_str(), alpha.id.as_str(), beta.id.as_str()]
    );
}

#[test]
fn rejects_incomplete_project_display_order() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let alpha = service
        .record_project(git_repo(&root, "alpha"))
        .expect("record alpha");
    service
        .record_project(git_repo(&root, "beta"))
        .expect("record beta");

    let error = service
        .reorder_projects(vec![alpha.id])
        .expect_err("incomplete order should fail");

    assert!(
        error
            .to_string()
            .contains("project order must include every project exactly once"),
        "unexpected error: {error}"
    );
}

#[test]
fn lists_project_as_stale_when_its_path_no_longer_exists() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "ghost");
    let recorded = service
        .record_project(repo.clone())
        .expect("record project");
    let recorded_path = recorded.path.clone();

    drop(root);

    let projects = service.list_projects().expect("list projects");
    let stale = projects
        .iter()
        .find(|p| p.id == recorded.id)
        .expect("project still listed");

    assert_eq!(stale.status, "stale");
    assert_eq!(stale.path, recorded_path);
}

#[test]
fn hidden_status_survives_even_when_path_no_longer_exists() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = ProjectService::new(db.clone());
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "buried");
    let recorded = service
        .record_project(repo.clone())
        .expect("record project");

    {
        let conn = db.connection().expect("get connection");
        conn.execute(
            "UPDATE projects SET status = 'hidden' WHERE id = ?1",
            params![recorded.id],
        )
        .expect("mark hidden");
    }

    drop(root);

    let projects = service.list_projects().expect("list projects");
    let hidden = projects
        .iter()
        .find(|p| p.id == recorded.id)
        .expect("project still listed");

    assert_eq!(hidden.status, "hidden");
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

#[test]
fn deletes_project_and_removes_it_from_list() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "doomed"))
        .expect("record project");

    service.delete_project(recorded.id).expect("delete project");

    assert!(service.list_projects().expect("list projects").is_empty());
}

#[test]
fn deleting_project_cascades_to_skills_and_sessions() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let service = ProjectService::new(db.clone());
    let root = TempDir::new().expect("create temp dir");
    let recorded = service
        .record_project(git_repo(&root, "cascaded"))
        .expect("record project");

    let conn = db.connection().expect("get connection");
    conn.execute(
        "INSERT INTO skills (id, name, scope, project_id, canonical_path, created_at, updated_at) \
         VALUES ('s1', 'skill-a', 'project', ?1, '/fake/skill-a', 0, 0)",
        params![recorded.id],
    )
    .expect("seed skill");
    conn.execute(
        "INSERT INTO session_index (id, project_id, title, file_path, source, updated_at) \
         VALUES ('se1', ?1, 'Session A', '/fake/session-a', 'local', 0)",
        params![recorded.id],
    )
    .expect("seed session");
    drop(conn);

    service.delete_project(recorded.id).expect("delete project");

    let conn = db.connection().expect("get connection");
    let skill_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM skills", [], |row| row.get(0))
        .expect("count skills");
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM session_index", [], |row| row.get(0))
        .expect("count sessions");
    assert_eq!(skill_count, 0);
    assert_eq!(session_count, 0);
}

#[test]
fn rejects_empty_project_id_on_delete() {
    let db = Database::open_in_memory().expect("open in-memory database");
    let service = ProjectService::new(db.into());

    let error = service
        .delete_project(String::new())
        .expect_err("empty id should fail");

    assert!(
        error.to_string().contains("project id is required"),
        "unexpected error: {error}"
    );
}
