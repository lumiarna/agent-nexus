// Declarative agent definitions — single source of truth.
// UI code and mock layer import from here; no computed derivation.

export interface AgentDirDef {
  envKey: string;
  value: string;
  /** The envKey this dir is derived from (e.g. "GENERIC_AGENT_CONFIG_DIR"). */
  derivedFrom?: string;
}

export type AgentSurface = "skill" | "prompt" | "provider";

export interface AgentDef<Name extends string = string> {
  name: Name;
  abbr: string;
  color: string;
  dirs: readonly AgentDirDef[];
  surfaces: readonly AgentSurface[];
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
    dirs: [
      { envKey: "GENERIC_AGENT_CONFIG_DIR", value: "~/.agents" },
      { envKey: "GENERIC_AGENT_SKILLS_DIR", value: "~/.agents/skills", derivedFrom: "GENERIC_AGENT_CONFIG_DIR" },
      { envKey: "GENERIC_AGENT_PROMPT_FILE", value: "~/.agents/AGENTS.md", derivedFrom: "GENERIC_AGENT_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Claude Code",
    abbr: "CC",
    color: "#c2410c",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { envKey: "CLAUDE_CODE_CONFIG_DIR", value: "~/.claude" },
      { envKey: "CLAUDE_CODE_SKILLS_DIR", value: "~/.claude/skills", derivedFrom: "CLAUDE_CODE_CONFIG_DIR" },
      { envKey: "CLAUDE_CODE_PROMPT_FILE", value: "~/.claude/CLAUDE.md", derivedFrom: "CLAUDE_CODE_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "CodeX",
    abbr: "CX",
    color: "#4f7a6a",
    authFile: "~/.codex/auth.json",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { envKey: "CODEX_CONFIG_DIR", value: "~/.codex" },
      { envKey: "CODEX_SKILLS_DIR", value: "~/.codex/skills", derivedFrom: "CODEX_CONFIG_DIR" },
      { envKey: "CODEX_PROMPT_FILE", value: "~/.codex/AGENTS.md", derivedFrom: "CODEX_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "Copilot",
    abbr: "CP",
    color: "#5a7894",
    authFile: "$GITHUB_TOKEN",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { envKey: "COPILOT_CONFIG_DIR", value: "~/.github" },
      { envKey: "COPILOT_SKILLS_DIR", value: "~/.github/skills", derivedFrom: "COPILOT_CONFIG_DIR" },
      { envKey: "COPILOT_PROMPT_FILE", value: "~/.github/AGENTS.md", derivedFrom: "COPILOT_CONFIG_DIR" },
    ],
  }),
  agent({
    name: "OpenCode",
    abbr: "OC",
    color: "#7a5c9e",
    authFile: "~/.local/share/opencode/auth.json",
    surfaces: ["skill", "prompt", "provider"],
    dirs: [
      { envKey: "OPENCODE_CONFIG_DIR", value: "~/.config/opencode" },
      { envKey: "OPENCODE_SKILLS_DIR", value: "~/.config/opencode/skills", derivedFrom: "OPENCODE_CONFIG_DIR" },
      { envKey: "OPENCODE_PROMPT_FILE", value: "~/.config/opencode/AGENTS.md", derivedFrom: "OPENCODE_CONFIG_DIR" },
    ],
  }),
] as const;

export type AgentName = (typeof AGENTS)[number]["name"];

/** Agent display order (derived from AGENTS). */
export const AGENT_ORDER: AgentName[] = AGENTS.map((a) => a.name);

export function agentHasSurface(agent: AgentName, surface: AgentSurface): boolean {
  return AGENTS.find((item) => item.name === agent)?.surfaces.includes(surface) ?? false;
}
