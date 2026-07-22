use std::{fs, io::ErrorKind, path::PathBuf};

use serde::Serialize;

use crate::{
    error::{AppError, AppResult},
    services::paths::resolve_local_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilitySurface {
    pub name: &'static str,
    pub abbr: &'static str,
    pub color: &'static str,
    pub config_dir: &'static str,
    pub skill: Option<SkillSurface>,
    pub prompt: Option<PromptSurface>,
    pub provider: Option<ProviderSurface>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSurface {
    pub global_dir: &'static str,
    pub project_dir: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSurface {
    pub global_file: &'static str,
    pub project_file: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSurface {
    pub provider_id: &'static str,
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
            project_file: Some("AGENTS.md"),
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
            project_file: Some("CLAUDE.md"),
        }),
        provider: Some(ProviderSurface {
            provider_id: "claude",
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
            project_file: None,
        }),
        provider: Some(ProviderSurface {
            provider_id: "codex",
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
            project_file: None,
        }),
        provider: Some(ProviderSurface {
            provider_id: "copilot",
            credential_hint: Some("settings.COPILOT_GITHUB_TOKEN"),
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
            project_file: None,
        }),
        provider: None,
    },
    AgentCapabilitySurface {
        name: "Pi",
        abbr: "PI",
        color: "#2563eb",
        config_dir: "~/.pi/agent",
        skill: Some(SkillSurface {
            global_dir: "~/.pi/agent/skills",
            project_dir: ".pi/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.pi/agent/AGENTS.md",
            project_file: None,
        }),
        provider: None,
    },
    AgentCapabilitySurface {
        name: "Qoder",
        abbr: "QO",
        color: "#0ea5e9",
        config_dir: "~/.qoder",
        skill: Some(SkillSurface {
            global_dir: "~/.qoder/skills",
            project_dir: ".qoder/skills",
        }),
        prompt: Some(PromptSurface {
            global_file: "~/.qoder/AGENTS.md",
            project_file: None,
        }),
        provider: Some(ProviderSurface {
            provider_id: "qoder",
            credential_hint: Some("manual qoder session cookie"),
        }),
    },
];

pub fn agent_capability_surfaces() -> &'static [AgentCapabilitySurface] {
    AGENT_CAPABILITY_SURFACES
}

pub fn list_agent_capability_surfaces() -> Vec<AgentCapabilitySurface> {
    AGENT_CAPABILITY_SURFACES.to_vec()
}

pub fn agent_by_name(name: &str) -> Option<&'static AgentCapabilitySurface> {
    AGENT_CAPABILITY_SURFACES
        .iter()
        .find(|agent| agent.name == name)
}

/// Resolve and validate a canonical Agent's configuration root.
pub fn resolve_agent_config_root(name: &str) -> AppResult<PathBuf> {
    let agent = agent_by_name(name)
        .ok_or_else(|| AppError::Validation(format!("unknown agent: {name}")))?;
    let config_root = resolve_local_path(agent.config_dir)?;
    let metadata = match fs::metadata(&config_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::Validation(format!(
                "config root for {name} does not exist: {}",
                config_root.display()
            )));
        }
        Err(error) => {
            return Err(AppError::Io(format!(
                "failed to inspect config root for {name} at {}: {error}",
                config_root.display()
            )));
        }
    };
    if !metadata.is_dir() {
        return Err(AppError::Validation(format!(
            "config root for {name} is not a directory: {}",
            config_root.display()
        )));
    }
    Ok(config_root)
}

pub fn agent_order_index(name: &str) -> Option<usize> {
    AGENT_CAPABILITY_SURFACES
        .iter()
        .position(|agent| agent.name == name)
}
