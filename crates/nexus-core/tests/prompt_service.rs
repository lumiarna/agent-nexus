use std::{env, fs, path::Path, sync::Arc};

use nexus_core::{
    database::Database,
    services::{
        agent_capabilities::agent_capability_surfaces,
        paths,
        prompts::{PromptService, SetPromptTargetInput},
        symlink::create_symlink_placement,
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

fn canonical_display_path(path: impl AsRef<Path>) -> String {
    let path = fs::canonicalize(path).expect("canonicalize path");
    paths::path_to_string(&path, "path").expect("display path")
}

#[cfg(unix)]
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
        assert_eq!(prompt.name, "Copilot Global Prompt");
        assert_eq!(prompt.path, canonical_display_path(source_file));
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
#[cfg(unix)]
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
        assert_link_points_to(&source_file, &target_file);

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
