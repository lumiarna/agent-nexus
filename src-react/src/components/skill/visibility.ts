// Pure visibility rules for Skill list surfaces. Kept dependency-free so the
// Global/Project filtering contract can be unit-tested under the NodeNext test
// build without React, Tauri, or alias-based imports.

export interface VisibleSkillListEntry {
  scope: "global" | "project";
  sourceKind?: "agent" | "project_custom";
  placementScope?: "project";
  cells: Record<string, string>;
}

function hasTargetPlacement(cells: Record<string, string>): boolean {
  return Object.values(cells).some((role) => role === "target");
}

/**
 * Global Skill page shows true global Skills plus canonical Project custom
 * source rows that currently have a managed Global placement. Incoming Project
 * placement projection rows must stay out of the Global page even though their
 * cells also contain `target`.
 */
export function showsInGlobalSkillPage(skill: VisibleSkillListEntry): boolean {
  return (
    skill.scope === "global" ||
    (skill.sourceKind === "project_custom" && !skill.placementScope && hasTargetPlacement(skill.cells))
  );
}
