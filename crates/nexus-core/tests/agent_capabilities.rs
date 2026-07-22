use std::{env, fs, panic, path::Path};

use nexus_core::services::agent_capabilities::{
    agent_by_name, agent_capability_surfaces, list_agent_capability_surfaces,
    resolve_agent_config_root,
};
use serial_test::serial;
use tempfile::TempDir;

fn with_home<F: FnOnce(&Path)>(home: &TempDir, f: F) {
    let previous_home = env::var_os("HOME");
    let previous_userprofile = env::var_os("USERPROFILE");
    env::set_var("HOME", home.path());
    env::set_var("USERPROFILE", home.path());
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| f(home.path())));
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
    match previous_userprofile {
        Some(value) => env::set_var("USERPROFILE", value),
        None => env::remove_var("USERPROFILE"),
    }
    if let Err(payload) = result {
        panic::resume_unwind(payload);
    }
}

#[test]
fn defines_agent_capability_surfaces_in_canonical_order() {
    let agents = agent_capability_surfaces();
    let names = agents.iter().map(|agent| agent.name).collect::<Vec<_>>();

    assert_eq!(
        names,
        [
            "Generic Agent",
            "Claude Code",
            "CodeX",
            "Copilot",
            "OpenCode",
            "Pi",
            "Qoder"
        ]
    );

    let generic = agent_by_name("Generic Agent").expect("generic agent capability");
    assert_eq!(generic.config_dir, "~/.agents");
    assert!(generic.skill.is_some());
    assert!(generic.prompt.is_some());
    assert_eq!(
        generic.prompt.expect("generic prompt surface").project_file,
        Some("AGENTS.md")
    );
    assert!(generic.provider.is_none());

    let claude = agent_by_name("Claude Code").expect("claude capability");
    assert_eq!(
        claude.prompt.expect("claude prompt surface").project_file,
        Some("CLAUDE.md")
    );

    let copilot = agent_by_name("Copilot").expect("copilot capability");
    assert_eq!(
        copilot.skill.expect("copilot skill surface").project_dir,
        ".github/skills"
    );
    assert_eq!(
        copilot.prompt.expect("copilot prompt surface").global_file,
        "~/.github/AGENTS.md"
    );
    assert_eq!(
        copilot.prompt.expect("copilot prompt surface").project_file,
        None
    );
    assert_eq!(
        copilot
            .provider
            .expect("copilot provider surface")
            .provider_id,
        "copilot"
    );
    assert_eq!(
        copilot
            .provider
            .expect("copilot provider surface")
            .credential_hint,
        Some("settings.COPILOT_GITHUB_TOKEN")
    );

    let codex = agent_by_name("CodeX").expect("codex capability");
    assert_eq!(
        codex.provider.expect("codex provider surface").provider_id,
        "codex"
    );
    assert_eq!(
        codex
            .provider
            .expect("codex provider surface")
            .credential_hint,
        Some("~/.codex/auth.json")
    );

    let pi = agent_by_name("Pi").expect("pi capability");
    assert_eq!(pi.config_dir, "~/.pi/agent");
    assert_eq!(
        pi.skill.expect("pi skill surface").global_dir,
        "~/.pi/agent/skills"
    );
    assert_eq!(
        pi.skill.expect("pi skill surface").project_dir,
        ".pi/skills"
    );
    assert_eq!(
        pi.prompt.expect("pi prompt surface").global_file,
        "~/.pi/agent/AGENTS.md"
    );
    assert_eq!(pi.prompt.expect("pi prompt surface").project_file, None);
    assert!(pi.provider.is_none());
}

#[test]
#[serial]
fn resolves_existing_config_roots_from_canonical_agent_names() {
    let home = tempfile::tempdir().expect("create temporary home");
    with_home(&home, |home| {
        for (name, relative_path) in [
            ("Generic Agent", ".agents"),
            ("Claude Code", ".claude"),
            ("CodeX", ".codex"),
            ("Copilot", ".github"),
            ("OpenCode", ".config/opencode"),
            ("Pi", ".pi/agent"),
            ("Qoder", ".qoder"),
        ] {
            let expected = home.join(relative_path);
            fs::create_dir_all(&expected).expect("create config root");
            assert_eq!(
                resolve_agent_config_root(name).expect("resolve canonical config root"),
                expected
            );
        }
    });

    let error = resolve_agent_config_root("untrusted/path").expect_err("reject unknown agent");
    assert!(error.to_string().contains("unknown agent: untrusted/path"));
}

#[test]
#[serial]
fn rejects_missing_config_root() {
    let home = tempfile::tempdir().expect("create temporary home");
    with_home(&home, |_| {
        let error = resolve_agent_config_root("Pi").expect_err("reject missing config root");

        assert!(error
            .to_string()
            .contains("config root for Pi does not exist"));
        assert!(error.to_string().contains(".pi"));
    });
}

#[test]
#[serial]
fn rejects_config_root_that_is_not_a_directory() {
    let home = tempfile::tempdir().expect("create temporary home");
    with_home(&home, |home| {
        fs::write(home.join(".agents"), "not a directory").expect("write config root file");

        let error = resolve_agent_config_root("Generic Agent")
            .expect_err("reject config root that is not a directory");

        assert!(error
            .to_string()
            .contains("config root for Generic Agent is not a directory"));
    });
}

#[test]
fn exposes_agent_backed_provider_surfaces_without_generic_agent() {
    let providers = list_agent_capability_surfaces()
        .into_iter()
        .filter_map(|agent| agent.provider.map(|provider| (agent.name, provider)))
        .collect::<Vec<_>>();

    assert_eq!(
        providers
            .iter()
            .map(|(name, provider)| (*name, provider.provider_id))
            .collect::<Vec<_>>(),
        [
            ("Claude Code", "claude"),
            ("CodeX", "codex"),
            ("Copilot", "copilot"),
            ("Qoder", "qoder")
        ]
    );
    assert!(!providers.iter().any(|(name, _)| *name == "Generic Agent"));
    assert!(!providers.iter().any(|(name, _)| *name == "OpenCode"));
}
