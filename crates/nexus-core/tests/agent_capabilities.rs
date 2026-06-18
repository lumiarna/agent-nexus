use nexus_core::services::agent_capabilities::{agent_by_name, agent_capability_surfaces};

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
            "OpenCode"
        ]
    );

    let generic = agent_by_name("Generic Agent").expect("generic agent capability");
    assert_eq!(generic.config_dir, "~/.agents");
    assert!(generic.skill.is_some());
    assert!(generic.prompt.is_some());
    assert!(generic.provider.is_none());

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
        copilot
            .provider
            .expect("copilot provider surface")
            .credential_hint,
        Some("$GITHUB_TOKEN")
    );

    let codex = agent_by_name("CodeX").expect("codex capability");
    assert_eq!(
        codex
            .provider
            .expect("codex provider surface")
            .credential_hint,
        Some("~/.codex/auth.json")
    );
}
