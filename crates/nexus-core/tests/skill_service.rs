use std::{env, fs, path::Path, sync::Arc};

use nexus_core::{
    database::Database,
    services::{
        paths,
        projects::ProjectService,
        skills::{SetSkillTargetInput, SkillService},
        symlink::create_managed_directory_link,
    },
};
use serial_test::serial;
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

fn create_directory_link(source: &Path, target: &Path) {
    create_managed_directory_link(source, target).expect("create directory link");
}

fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    paths::path_to_string(&path, "path").expect("display path")
}

fn assert_link_points_to(source: &Path, target: &Path) {
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

#[test]
#[serial]
fn scans_project_skills_and_derives_distribution_from_links() {
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
    create_directory_link(&source_dir, &target_dir);
    fs::create_dir_all(ignored_symlink_source.parent().unwrap()).expect("create linked parent");
    create_directory_link(&source_dir, &ignored_symlink_source);

    let rows = skills.scan_skills().expect("scan skills");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "tap-builder");
    assert_eq!(rows[0].desc, "Project-scoped TAP scaffolder");
    assert_eq!(rows[0].scope, "project");
    assert_eq!(rows[0].project_id.as_deref(), Some(project.id.as_str()));
    assert!(rows[0].disabled);
    assert_eq!(rows[0].path, canonical_display_path(source_dir));
    assert_eq!(rows[0].cells["Copilot"], "source");
    assert_eq!(rows[0].cells["CodeX"], "target");
    assert_eq!(rows[0].cells["Claude Code"], "none");
}

#[test]
#[serial]
fn lists_project_skills_by_project_display_order() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);
    let alpha_repo = git_repo(&root, "alpha");
    let beta_repo = git_repo(&root, "beta");
    let alpha = projects
        .record_project(alpha_repo.clone())
        .expect("record alpha project");
    let beta = projects
        .record_project(beta_repo.clone())
        .expect("record beta project");
    projects
        .reorder_projects(vec![beta.id.clone(), alpha.id.clone()])
        .expect("reorder projects");

    write_skill(
        &Path::new(&alpha_repo).join(".codex/skills/alpha-skill"),
        r#"---
name: alpha-skill
description: Alpha project skill
---

# Alpha Skill
"#,
    );
    write_skill(
        &Path::new(&beta_repo).join(".codex/skills/zeta-skill"),
        r#"---
name: zeta-skill
description: Beta project skill
---

# Zeta Skill
"#,
    );

    let rows = skills.scan_skills().expect("scan skills");
    let project_ids = rows
        .iter()
        .filter(|skill| skill.scope == "project")
        .map(|skill| skill.project_id.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        project_ids,
        vec![Some(beta.id.as_str()), Some(alpha.id.as_str())]
    );
}

#[test]
#[serial]
fn toggles_project_distribution_target_link() {
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
    assert_link_points_to(&source_dir, &target_dir);

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
#[serial]
fn scans_project_custom_skills_dir_as_sourceless_skill() {
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
    // `skills` is the default custom dir — no extra configuration needed.
    let custom_dir = Path::new(&repo).join("skills/release-notes");

    write_skill(
        &custom_dir,
        r#"---
name: release-notes
description: Draft project release notes
---

# Release Notes
"#,
    );

    let rows = skills.scan_skills().expect("scan skills");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "release-notes");
    assert_eq!(rows[0].scope, "project");
    assert_eq!(rows[0].project_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(rows[0].source_kind, "project_custom");
    assert_eq!(rows[0].source_agent, None);
    // No Agent source cell — every Agent is `none` until propagated.
    assert!(rows[0].cells.values().all(|role| role == "none"));
}

#[test]
#[serial]
fn propagates_project_custom_skill_to_global_and_keeps_single_source() {
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
    let custom_dir = Path::new(&repo).join("skills/release-notes");

    write_skill(
        &custom_dir,
        r#"---
name: release-notes
description: Draft project release notes
---

# Release Notes
"#,
    );
    let scanned = skills.scan_skills().expect("scan skills");
    let skill_id = scanned[0].id.clone();

    // Propagate to Global through the default entry Agent.
    let propagated = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: skill_id.clone(),
            agent: "Generic Agent".to_string(),
            enabled: true,
        })
        .expect("propagate to Global");
    assert_eq!(propagated.cells["Generic Agent"], "target");
    let generic_link = home.join(".agents/skills/release-notes");
    assert_link_points_to(&custom_dir, &generic_link);

    // Fan out to another Global Agent.
    let fanned = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: skill_id.clone(),
            agent: "Claude Code".to_string(),
            enabled: true,
        })
        .expect("fan out to Claude Code");
    assert_eq!(fanned.cells["Claude Code"], "target");
    assert_link_points_to(&custom_dir, &home.join(".claude/skills/release-notes"));

    // A rescan must not turn the Global placement symlinks into new canonical skills.
    let rescanned = skills.scan_skills().expect("rescan skills");
    assert_eq!(rescanned.len(), 1);
    assert_eq!(rescanned[0].source_kind, "project_custom");
    assert_eq!(rescanned[0].cells["Generic Agent"], "target");
    assert_eq!(rescanned[0].cells["Claude Code"], "target");

    // Out-of-band removal of a Global placement falls back to `none` on rescan.
    #[cfg(unix)]
    std::fs::remove_file(&generic_link).expect("remove placement symlink");
    #[cfg(windows)]
    std::fs::remove_dir(&generic_link).expect("remove placement junction");

    let after_removal = skills.scan_skills().expect("rescan after removal");
    assert_eq!(after_removal.len(), 1);
    assert_eq!(after_removal[0].cells["Generic Agent"], "none");
    assert_eq!(after_removal[0].cells["Claude Code"], "target");
}

#[test]
#[serial]
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
