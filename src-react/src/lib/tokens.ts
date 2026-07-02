// Runtime design tokens — colors that are *computed* (agent colors, status
// tints with alpha suffixes, quota thresholds) and therefore consumed via inline
// `style`, not Tailwind classes. Mirrors prototype/nexus-data.js.

import type { AgentName, CellRole, Cells, ProviderStatus, Skill } from "@/types";
import { AGENTS, AGENT_ORDER } from "@/config/agents";

export { AGENT_ORDER } from "@/config/agents";

export function agentAbbr(a: AgentName): string {
  return AGENTS.find((x) => x.name === a)?.abbr ?? "?";
}

export function agentColor(a: AgentName): string {
  return AGENTS.find((x) => x.name === a)?.color ?? "#a99a89";
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

/** Brand identity (display name + colour) for a Provider id, used to paint its
 *  Windows-taskbar tray icon. Agent-backed providers reuse their Agent colour;
 *  others fall back to the neutral accent so an enabled tray icon always renders. */
export function providerBrand(providerId: string): { name: string; color: string } {
  const agent = AGENTS.find((a) => a.providerId === providerId);
  return agent
    ? { name: agent.name, color: agent.color }
    : { name: providerId, color: palette.accent };
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

/** First agent holding the canonical source in a matrix row (fallback: leftmost). */
export function srcAgentOf(cells: Cells): AgentName {
  for (const a of AGENT_ORDER) if (cells[a] === "source") return a;
  return AGENT_ORDER[0];
}

/** A Project custom Skill has its canonical source outside any Agent skills dir,
 *  so its Agent Matrix row carries no `source` cell. */
export function isProjectCustomSkill(skill: Pick<Skill, "sourceKind">): boolean {
  return skill.sourceKind === "project_custom";
}

/** Whether a Skill currently has at least one Agent placement (target cell).
 *  For a Project custom Skill this means it has been propagated to Global. */
export function hasGlobalPlacement(cells: Cells): boolean {
  return AGENT_ORDER.some((a) => cells[a] === "target");
}

/** Agents currently holding a placement (target cell), in canonical order. */
export function targetAgentsOf(cells: Cells): AgentName[] {
  return AGENT_ORDER.filter((a) => cells[a] === "target");
}

/** Toggle a matrix cell target↔none. The source cell is fixed (no-op). */
export function toggleCellRole(cells: Cells, agent: AgentName): Cells {
  if (cells[agent] === "source") return cells;
  const next: CellRole = cells[agent] === "target" ? "none" : "target";
  return { ...cells, [agent]: next };
}
