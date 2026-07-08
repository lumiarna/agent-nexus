// Declarative agent definitions for browser preview and TypeScript literals.
// The desktop runtime consumes the backend Agent Capability Surface.

export interface AgentDirDef {
  key: string;
  value: string;
  /** The config key this dir is derived from (e.g. "GENERIC_AGENT_CONFIG_DIR"). */
  derivedFrom?: string;
}

export type AgentSurface = "skill" | "prompt" | "provider";

export interface AgentDef<Name extends string = string> {
  name: Name;
  abbr: string;
  color: string;
  dirs: readonly AgentDirDef[];
  surfaces: readonly AgentSurface[];
  projectSkillDir?: string;
  projectPromptFile?: string;
  providerId?: string;
  /** Credential file path, if any. Not a config root — used by Provider logic. */
  authFile?: string;
}

function agent<const Name extends string>(definition: AgentDef<Name>): AgentDef<Name> {
  return definition;
}

/**
 * Canonical agent list in display/distribution order.
 * "Generic Agent" sits leftmost in the Agent Matrix.
 */
export const AGENTS = [
  agent({
    name: "Generic Agent",
    abbr: "AG",
    color: "#9a7b53",
    surfaces: ["skill", "prompt"],
    projectSkillDir: ".agents/skills",
    projectPromptFile: "AGENTS.md",
    dirs: [
      { key: "GENERIC_AGENT_CONFIG_DIR", value: "~/.agents" },
      { key: "GENERIC_AGENT_SKILLS_DIR", value: "~/.agents/skills", derivedFrom: "GENERIC_AGENT_CONFIG_DIR" },
      { key: "GENERIC_AGENT_PROMPT_FILE", value: "~/.agents/AGENTS.md", derivedFrom: "GENERIC_AGENT_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Claude Code",
    abbr: "CC",
    color: "#c2410c",
    surfaces: ["skill", "prompt", "provider"],
    projectSkillDir: ".claude/skills",
    projectPromptFile: "CLAUDE.md",
    providerId: "claude",
    authFile: "~/.claude",
    dirs: [
      { key: "CLAUDE_CONFIG_DIR", value: "~/.claude" },
      { key: "CLAUDE_SKILLS_DIR", value: "~/.claude/skills", derivedFrom: "CLAUDE_CONFIG_DIR" },
      { key: "CLAUDE_PROMPT_FILE", value: "~/.claude/CLAUDE.md", derivedFrom: "CLAUDE_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "CodeX",
    abbr: "CX",
    color: "#4f7a6a",
    projectSkillDir: ".codex/skills",
    providerId: "codex",
    authFile: "~/.codex/auth.json",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { key: "CODEX_CONFIG_DIR", value: "~/.codex" },
      { key: "CODEX_SKILLS_DIR", value: "~/.codex/skills", derivedFrom: "CODEX_CONFIG_DIR" },
      { key: "CODEX_PROMPT_FILE", value: "~/.codex/AGENTS.md", derivedFrom: "CODEX_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Copilot",
    abbr: "CP",
    color: "#5a7894",
    projectSkillDir: ".github/skills",
    providerId: "copilot",
    authFile: "settings.COPILOT_GITHUB_TOKEN",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { key: "COPILOT_CONFIG_DIR", value: "~/.github" },
      { key: "COPILOT_SKILLS_DIR", value: "~/.github/skills", derivedFrom: "COPILOT_CONFIG_DIR" },
      { key: "COPILOT_PROMPT_FILE", value: "~/.github/AGENTS.md", derivedFrom: "COPILOT_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "OpenCode",
    abbr: "OC",
    color: "#8b5cf6",
    projectSkillDir: ".opencode/skills",
    surfaces: ["skill", "prompt"],
    dirs: [
      { key: "OPENCODE_CONFIG_DIR", value: "~/.config/opencode" },
      { key: "OPENCODE_SKILLS_DIR", value: "~/.config/opencode/skills", derivedFrom: "OPENCODE_CONFIG_DIR" },
      { key: "OPENCODE_PROMPT_FILE", value: "~/.config/opencode/AGENTS.md", derivedFrom: "OPENCODE_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Pi",
    abbr: "PI",
    color: "#2563eb",
    projectSkillDir: ".pi/skills",
    surfaces: ["skill", "prompt"],
    dirs: [
      { key: "PI_CONFIG_DIR", value: "~/.pi/agent" },
      { key: "PI_SKILLS_DIR", value: "~/.pi/agent/skills", derivedFrom: "PI_CONFIG_DIR" },
      { key: "PI_PROMPT_FILE", value: "~/.pi/agent/AGENTS.md", derivedFrom: "PI_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Qoder",
    abbr: "QO",
    color: "#0ea5e9",
    projectSkillDir: ".qoder/skills",
    surfaces: ["skill", "prompt", "provider"],
    providerId: "qoder",
    authFile: "manual qoder session cookie",
    dirs: [
      { key: "QODER_CONFIG_DIR", value: "~/.qoder" },
      { key: "QODER_SKILLS_DIR", value: "~/.qoder/skills", derivedFrom: "QODER_CONFIG_DIR" },
      { key: "QODER_PROMPT_FILE", value: "~/.qoder/AGENTS.md", derivedFrom: "QODER_CONFIG_DIR" },
    ],
  }),
] as const;

export type AgentName = (typeof AGENTS)[number]["name"];

/** Agent display order (derived from AGENTS). */
export const AGENT_ORDER: AgentName[] = AGENTS.map((a) => a.name);

export function agentHasSurface(agent: AgentName, surface: AgentSurface): boolean {
  return AGENTS.find((item) => item.name === agent)?.surfaces.includes(surface) ?? false;
}
