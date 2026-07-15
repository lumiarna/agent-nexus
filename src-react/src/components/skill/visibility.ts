import type { Skill } from "../../types/index.js";

function hasTargetPlacement(cells: Record<string, string>): boolean {
  return Object.values(cells).some((role) => role === "target");
}

/** Global shows true Global Agent canonical rows plus Project custom canonical
 * rows whose single eager Global destination has a live Placement. Incoming
 * Project rows are excluded by construction. */
export function showsInGlobalSkillPage(skill: Skill): boolean {
  switch (skill.kind) {
    case "agentCanonical":
      return skill.context.kind === "global";
    case "projectCustomCanonical": {
      const global = skill.destinations.find((target) => target.kind === "global");
      return global ? hasTargetPlacement(global.cells) : false;
    }
    case "projectCustomIncoming":
      return false;
  }
}

export function projectForSkillRow(skill: Skill): { id: string; name: string } | undefined {
  switch (skill.kind) {
    case "agentCanonical":
      return skill.context.kind === "project" ? skill.context.project : undefined;
    case "projectCustomCanonical":
      return skill.sourceProject;
    case "projectCustomIncoming":
      return skill.targetProject;
  }
}
