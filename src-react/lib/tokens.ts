// Runtime design tokens — colors that are *computed* (agent colors, status
// tints with alpha suffixes, quota thresholds) and therefore consumed via inline
// `style`, not Tailwind classes. Mirrors prototype/nexus-data.js.

import type { AgentName, ProviderStatus } from "@/types";

/** Canonical agent display/distribution order. "Agents" is the generic ~/.agents
 *  default and sits leftmost in the Agent Matrix. */
export const AGENT_ORDER: AgentName[] = [
  "Agents",
  "Claude Code",
  "CodeX",
  "Copilot",
  "OpenCode",
];

interface AgentMetaEntry {
  abbr: string;
  color: string;
  configDir: string;
  generic?: boolean;
}

export const AGENT_META: Record<AgentName, AgentMetaEntry> = {
  Agents: { abbr: "AG", color: "#9a7b53", configDir: "~/.agents", generic: true },
  "Claude Code": { abbr: "CC", color: "#c2410c", configDir: "~/.claude" },
  CodeX: { abbr: "CX", color: "#4f7a6a", configDir: "~/.codex" },
  Copilot: { abbr: "CP", color: "#5a7894", configDir: "~/.github" },
  OpenCode: { abbr: "OC", color: "#7a5c9e", configDir: "~/.config/opencode" },
};

export function agentAbbr(a: AgentName): string {
  return AGENT_META[a]?.abbr ?? "?";
}

export function agentColor(a: AgentName): string {
  return AGENT_META[a]?.color ?? "#a99a89";
}

/** Status/accent palette. */
export const palette = {
  good: "#8a9a5b",
  warn: "#c2913f",
  crit: "#b55440",
  accent: "#9d7a64",
  muted: "#a99a89",
} as const;

/** Quota bar color by used-percent threshold. */
export function quotaColor(used: number): string {
  return used >= 90 ? palette.crit : used >= 70 ? palette.warn : palette.good;
}

/** Provider status (incl. the transient "loading" UI state) → label + color. */
export type ProviderUiStatus = ProviderStatus | "loading";

export function statusInfo(st: ProviderUiStatus): { label: string; color: string } {
  switch (st) {
    case "available":
      return { label: "Available", color: palette.good };
    case "expired":
      return { label: "Token expired", color: palette.warn };
    case "failed":
      return { label: "Request failed", color: palette.crit };
    case "loading":
      return { label: "Checking…", color: palette.accent };
    default:
      return { label: "No credentials", color: palette.muted };
  }
}
