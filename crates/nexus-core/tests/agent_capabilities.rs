use nexus_core::services::agent_capabilities::{
    agent_by_name, agent_capability_surfaces, list_agent_capability_surfaces,
};

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
    assert_eq!(pi.skill.expect("pi skill surface").global_dir, "~/.pi/agent/skills");
    assert_eq!(pi.skill.expect("pi skill surface").project_dir, ".pi/skills");
    assert_eq!(pi.prompt.expect("pi prompt surface").global_file, "~/.pi/agent/AGENTS.md");
    assert_eq!(pi.prompt.expect("pi prompt surface").project_file, None);
    assert!(pi.provider.is_none());
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
