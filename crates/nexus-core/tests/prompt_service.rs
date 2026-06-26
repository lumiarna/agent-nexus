use std::{env, fs, path::Path, sync::Arc};

#[cfg(unix)]
use nexus_core::services::{
    agent_capabilities::agent_capability_surfaces, paths, symlink::create_symlink_placement,
};
use nexus_core::{
    database::Database,
    services::{
        projects::ProjectService,
        prompts::{PromptService, SetPromptTargetInput},
    },
};
use serial_test::serial;
use tempfile::TempDir;

fn with_isolated_home<F: FnOnce(&Path)>(f: F) {
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    let previous_home = env::var_os("HOME");
    let previous_userprofile = env::var_os("USERPROFILE");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&home)));

    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
    match previous_userprofile {
        Some(value) => env::set_var("USERPROFILE", value),
        None => env::remove_var("USERPROFILE"),
    }

    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn write_prompt(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("prompt parent")).expect("create prompt parent");
    fs::write(path, body).expect("write prompt");
}

fn git_repo(parent: &Path, name: &str) -> String {
    let path = parent.join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    paths::path_to_string(&path, "path").expect("display path")
}

fn assert_file_distribution_tracks_source_writes(source: &Path, target: &Path) {
    assert_eq!(
        fs::read_to_string(target).expect("read target prompt"),
        fs::read_to_string(source).expect("read source prompt")
    );

    fs::write(source, "# Updated instructions\n").expect("update source prompt");

    assert_eq!(
        fs::read_to_string(target).expect("read updated target prompt"),
        "# Updated instructions\n"
    );
}

#[test]
#[serial]
#[cfg(unix)]
fn scans_global_prompt_sources_and_derives_distribution_from_capability_surface() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        let source_file = home.join(".github/AGENTS.md");
        let target_file = home.join(".codex/AGENTS.md");
        write_prompt(&source_file, "# Copilot instructions\n");
        fs::create_dir_all(target_file.parent().expect("target parent"))
            .expect("create target parent");
        create_symlink_placement(&source_file, &target_file).expect("create prompt link");

        let rows = prompts.scan_prompts().expect("scan prompts");

        assert_eq!(rows.len(), 1);
        let prompt = &rows[0];
        assert_eq!(prompt.name, "AGENTS.md");
        assert_eq!(prompt.scope, "global");
        assert_eq!(prompt.project_id, None);
        assert_eq!(prompt.path, canonical_display_path(source_file));
        assert_eq!(prompt.content, "# Copilot instructions\n");
        assert_eq!(prompt.cells.len(), agent_capability_surfaces().len());
        for agent in agent_capability_surfaces() {
            assert!(prompt.cells.contains_key(agent.name));
        }
        assert_eq!(prompt.cells["Copilot"], "source");
        assert_eq!(prompt.cells["CodeX"], "target");
        assert_eq!(prompt.cells["Generic Agent"], "none");
        assert_eq!(prompt.cells["Claude Code"], "none");
        assert_eq!(prompt.cells["OpenCode"], "none");
    });
}

#[test]
#[serial]
fn lists_global_prompt_sources_in_capability_order_not_prompt_name_order() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        write_prompt(&home.join(".codex/AGENTS.md"), "# CodeX instructions\n");
        write_prompt(&home.join(".claude/CLAUDE.md"), "# Claude instructions\n");

        let rows = prompts.scan_prompts().expect("scan prompts");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells["Claude Code"], "source");
        assert_eq!(rows[0].name, "CLAUDE.md");
        assert_eq!(rows[1].cells["CodeX"], "source");
        assert_eq!(rows[1].name, "AGENTS.md");
    });
}

#[test]
#[serial]
#[cfg(unix)]
fn scans_project_prompt_sources_with_two_agent_matrix_and_content() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        let repo = git_repo(&home.join("workspace"), "agent-nexus");
        let project = projects
            .record_project(repo.clone())
            .expect("record project");
        let repo = Path::new(&repo);
        let source_file = repo.join("AGENTS.md");
        let target_file = repo.join("CLAUDE.md");
        write_prompt(&source_file, "# Project instructions\n");
        create_symlink_placement(&source_file, &target_file).expect("create project prompt link");

        let rows = prompts.scan_prompts().expect("scan project prompts");

        assert_eq!(rows.len(), 1);
        let prompt = &rows[0];
        assert_eq!(prompt.name, "agent-nexus · AGENTS.md");
        assert_eq!(prompt.scope, "project");
        assert_eq!(prompt.project_id.as_deref(), Some(project.id.as_str()));
        assert_eq!(prompt.path, canonical_display_path(source_file));
        assert_eq!(prompt.content, "# Project instructions\n");
        assert_eq!(prompt.cells["Generic Agent"], "source");
        assert_eq!(prompt.cells["Claude Code"], "target");
        assert_eq!(prompt.cells["CodeX"], "none");
        assert_eq!(prompt.cells["Copilot"], "none");
        assert_eq!(prompt.cells["OpenCode"], "none");
    });
}

#[test]
#[serial]
fn scans_both_real_project_prompt_files_as_independent_sources() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        let repo = git_repo(&home.join("workspace"), "agent-nexus");
        projects
            .record_project(repo.clone())
            .expect("record project");
        let repo = Path::new(&repo);
        write_prompt(&repo.join("AGENTS.md"), "# Generic instructions\n");
        write_prompt(&repo.join("CLAUDE.md"), "# Claude instructions\n");

        let rows = prompts.scan_prompts().expect("scan project prompts");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "agent-nexus · AGENTS.md");
        assert_eq!(rows[0].content, "# Generic instructions\n");
        assert_eq!(rows[0].cells["Generic Agent"], "source");
        assert_eq!(rows[0].cells["Claude Code"], "none");
        assert_eq!(rows[1].name, "agent-nexus · CLAUDE.md");
        assert_eq!(rows[1].content, "# Claude instructions\n");
        assert_eq!(rows[1].cells["Generic Agent"], "none");
        assert_eq!(rows[1].cells["Claude Code"], "source");
    });
}

#[test]
#[serial]
fn toggles_project_prompt_distribution_and_rejects_non_project_agents() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        let repo = git_repo(&home.join("workspace"), "agent-nexus");
        projects
            .record_project(repo.clone())
            .expect("record project");
        let repo = Path::new(&repo);
        let source_file = repo.join("AGENTS.md");
        let target_file = repo.join("CLAUDE.md");
        write_prompt(&source_file, "# Project instructions\n");
        let scanned = prompts.scan_prompts().expect("scan project prompt");

        let error = prompts
            .set_prompt_target(SetPromptTargetInput {
                prompt_id: scanned[0].id.clone(),
                agent: "CodeX".to_string(),
                enabled: true,
            })
            .expect_err("CodeX must not be a project prompt target");
        assert!(error
            .to_string()
            .contains("agent does not support prompt targets in this scope"));
        assert!(!repo.join(".codex/AGENTS.md").exists());

        let enabled = prompts
            .set_prompt_target(SetPromptTargetInput {
                prompt_id: scanned[0].id.clone(),
                agent: "Claude Code".to_string(),
                enabled: true,
            })
            .expect("enable Claude Code target");

        assert_eq!(enabled.cells["Claude Code"], "target");
        assert_file_distribution_tracks_source_writes(&source_file, &target_file);

        let disabled = prompts
            .set_prompt_target(SetPromptTargetInput {
                prompt_id: scanned[0].id.clone(),
                agent: "Claude Code".to_string(),
                enabled: false,
            })
            .expect("disable Claude Code target");

        assert_eq!(disabled.cells["Claude Code"], "none");
        assert!(!target_file.exists());
    });
}

#[test]
#[serial]
fn toggles_global_prompt_distribution_target_link() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let prompts = PromptService::new(db);

    with_isolated_home(|home| {
        let source_file = home.join(".github/AGENTS.md");
        let target_file = home.join(".codex/AGENTS.md");
        write_prompt(&source_file, "# Copilot instructions\n");
        let scanned = prompts.scan_prompts().expect("scan prompts");

        let enabled = prompts
            .set_prompt_target(SetPromptTargetInput {
                prompt_id: scanned[0].id.clone(),
                agent: "CodeX".to_string(),
                enabled: true,
            })
            .expect("enable CodeX target");

        assert_eq!(enabled.cells["CodeX"], "target");
        assert_file_distribution_tracks_source_writes(&source_file, &target_file);

        let disabled = prompts
            .set_prompt_target(SetPromptTargetInput {
                prompt_id: scanned[0].id.clone(),
                agent: "CodeX".to_string(),
                enabled: false,
            })
            .expect("disable CodeX target");

        assert_eq!(disabled.cells["CodeX"], "none");
        assert!(!target_file.exists());
    });
}
