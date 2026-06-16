use std::{env, fs, path::Path, sync::Arc};

use agent_nexus_lib::{
    database::Database,
    services::{
        projects::ProjectService,
        skills::{SetSkillTargetInput, SkillService},
    },
};
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn write_skill(dir: &Path, body: &str) {
    fs::create_dir_all(dir).expect("create skill dir");
    fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
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
fn scans_project_skills_and_derives_distribution_from_symlinks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);
    let repo = git_repo(&root, "agent-nexus");
    let project = projects
        .record_project(repo.clone())
        .expect("record project");
    let repo = Path::new(&repo);
    let source_dir = repo.join(".github/skills/tap-builder");
    let target_dir = repo.join(".codex/skills/tap-builder");
    let ignored_symlink_source = repo.join(".claude/skills/linked-only");

    write_skill(
        &source_dir,
        r#"---
name: tap-builder
description: Project-scoped TAP scaffolder
disable-model-invocation: true
---

# Tap Builder
"#,
    );
    fs::create_dir_all(target_dir.parent().unwrap()).expect("create target parent");
    symlink_dir(&source_dir, &target_dir);
    fs::create_dir_all(ignored_symlink_source.parent().unwrap()).expect("create linked parent");
    symlink_dir(&source_dir, &ignored_symlink_source);

    let rows = skills.scan_skills().expect("scan skills");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "tap-builder");
    assert_eq!(rows[0].desc, "Project-scoped TAP scaffolder");
    assert_eq!(rows[0].scope, "project");
    assert_eq!(rows[0].project_id.as_deref(), Some(project.id.as_str()));
    assert!(rows[0].disabled);
    assert_eq!(
        rows[0].path,
        fs::canonicalize(source_dir).unwrap().to_string_lossy()
    );
    assert_eq!(rows[0].cells["Copilot"], "source");
    assert_eq!(rows[0].cells["CodeX"], "target");
    assert_eq!(rows[0].cells["Claude Code"], "none");
}

#[test]
fn toggles_project_distribution_target_symlink() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);
    let repo = git_repo(&root, "agent-nexus");
    projects
        .record_project(repo.clone())
        .expect("record project");
    let repo = Path::new(&repo);
    let source_dir = repo.join(".github/skills/tap-builder");
    let target_dir = repo.join(".codex/skills/tap-builder");

    write_skill(
        &source_dir,
        r#"---
name: tap-builder
description: Project-scoped TAP scaffolder
---

# Tap Builder
"#,
    );
    let scanned = skills.scan_skills().expect("scan skills");

    let enabled = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: scanned[0].id.clone(),
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect("enable CodeX target");

    assert_eq!(enabled.cells["CodeX"], "target");
    assert_eq!(
        fs::read_link(&target_dir).expect("read target symlink"),
        fs::canonicalize(&source_dir).expect("canonicalize source")
    );

    let disabled = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: scanned[0].id.clone(),
            agent: "CodeX".to_string(),
            enabled: false,
        })
        .expect("disable CodeX target");

    assert_eq!(disabled.cells["CodeX"], "none");
    assert!(!target_dir.exists());
}

#[test]
fn toggles_disable_model_invocation_in_skill_file() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);
    let repo = git_repo(&root, "agent-nexus");
    projects
        .record_project(repo.clone())
        .expect("record project");
    let skill_dir = Path::new(&repo).join(".codex/skills/test-runner");

    write_skill(
        &skill_dir,
        r#"---
name: test-runner
description: Run project tests
---

# Test Runner
"#,
    );
    let scanned = skills.scan_skills().expect("scan skills");

    let disabled = skills
        .set_skill_disabled(scanned[0].id.clone(), true)
        .expect("disable model invocation");

    assert!(disabled.disabled);
    assert!(fs::read_to_string(skill_dir.join("SKILL.md"))
        .expect("read SKILL.md")
        .contains("disable-model-invocation: true"));

    let enabled = skills
        .set_skill_disabled(scanned[0].id.clone(), false)
        .expect("enable model invocation");

    assert!(!enabled.disabled);
    assert!(fs::read_to_string(skill_dir.join("SKILL.md"))
        .expect("read SKILL.md")
        .contains("disable-model-invocation: false"));
}
