use std::{fs, path::Path, sync::Arc};

use nexus_core::{
    database::Database,
    services::{projects::ProjectService, sessions::SessionService},
};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
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
