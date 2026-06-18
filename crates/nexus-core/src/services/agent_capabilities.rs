#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentCapabilitySurface {
    pub name: &'static str,
    pub abbr: &'static str,
    pub color: &'static str,
    pub config_dir: &'static str,
    pub skill: Option<SkillSurface>,
    pub prompt: Option<PromptSurface>,
    pub provider: Option<ProviderSurface>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillSurface {
    pub global_dir: &'static str,
    pub project_dir: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptSurface {
    pub global_file: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSurface {
    pub credential_hint: Option<&'static str>,
}

const AGENT_CAPABILITY_SURFACES: &[AgentCapabilitySurface] = &[
    AgentCapabilitySurface {
        name: "Generic Agent",
        abbr: "AG",
        color: "#9a7b53",
        config_dir: "~/.agents",
        skill: Some(SkillSurface {
            global_dir: "~/.agents/skills",
            project_dir: ".agents/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.agents/AGENTS.md",
        }),
        provider: None,
    },
    AgentCapabilitySurface {
        name: "Claude Code",
        abbr: "CC",
        color: "#c2410c",
        config_dir: "~/.claude",
        skill: Some(SkillSurface {
            global_dir: "~/.claude/skills",
            project_dir: ".claude/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.claude/CLAUDE.md",
        }),
        provider: Some(ProviderSurface {
            credential_hint: Some("~/.claude"),
        }),
    },
    AgentCapabilitySurface {
        name: "CodeX",
        abbr: "CX",
        color: "#4f7a6a",
        config_dir: "~/.codex",
        skill: Some(SkillSurface {
            global_dir: "~/.codex/skills",
            project_dir: ".codex/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.codex/AGENTS.md",
        }),
        provider: Some(ProviderSurface {
            credential_hint: Some("~/.codex/auth.json"),
        }),
    },
    AgentCapabilitySurface {
        name: "Copilot",
        abbr: "CP",
        color: "#5a7894",
        config_dir: "~/.github",
        skill: Some(SkillSurface {
            global_dir: "~/.github/skills",
            project_dir: ".github/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.github/AGENTS.md",
        }),
        provider: Some(ProviderSurface {
            credential_hint: Some("$GITHUB_TOKEN"),
        }),
    },
    AgentCapabilitySurface {
        name: "OpenCode",
        abbr: "OC",
        color: "#7a5c9e",
        config_dir: "~/.config/opencode",
        skill: Some(SkillSurface {
            global_dir: "~/.config/opencode/skills",
            project_dir: ".opencode/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.config/opencode/AGENTS.md",
        }),
        provider: Some(ProviderSurface {
            credential_hint: Some("~/.local/share/opencode/auth.json"),
        }),
    },
];

pub fn agent_capability_surfaces() -> &'static [AgentCapabilitySurface] {
    AGENT_CAPABILITY_SURFACES
}

pub fn agent_by_name(name: &str) -> Option<&'static AgentCapabilitySurface> {
    AGENT_CAPABILITY_SURFACES
        .iter()
        .find(|agent| agent.name == name)
}
