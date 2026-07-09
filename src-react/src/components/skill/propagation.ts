// Pure helpers for the Project custom Skill propagation menu. Kept dependency-
// free (only types + tokens) so it can be unit-tested like `sync/taskRules.ts`.

import {
  hasGlobalPlacement,
  isProjectCustomSkill,
  targetAgentsOf,
} from "@/lib/tokens";
import type { AgentName, Skill } from "@/types";

export interface PropagationTargetProject {
  id: string;
  name: string;
}

/** One selectable target in the "Propagate to…" modal. */
export interface PropagationTarget {
  /** `global` = the existing Propagate-to-Global flow; `project` = cross-Project. */
  kind: "global" | "project";
  /** Target Project id. Present only for `kind === "project"`. */
  projectId?: string;
  /** Display name — "Global" or the target Project name. */
  projectName: string;
  /** Currently has at least one live placement in this target. */
  enabled: boolean;
  /** Default entry Agent (from Settings) used when first propagating. */
  defaultAgent: AgentName;
  /** Agents currently holding a placement in this target (canonical order). */
  targetAgents: AgentName[];
}

/**
 * Compute the modal target list for a Project custom Skill: Global + every
 * active Project, including the source Project. Each entry carries its current
 * enabled state, derived from the canonical row's Global cells (for Global) or
 * the matching incoming projection row (for a target Project). Foreign
 * projection rows are matched by `canonicalSkillId` + `placementProjectId`, so
 * the source row stays the single canonical source and never carries
 * per-Project state itself.
 */
export function computePropagationTargets(
  skill: Skill,
  allSkills: Skill[],
  projects: PropagationTargetProject[],
  defaultGlobalEntry: AgentName,
): PropagationTarget[] {
  if (!isProjectCustomSkill(skill)) return [];

  const globalTargets = targetAgentsOf(skill.cells);
  const targets: PropagationTarget[] = [
    {
      kind: "global",
      projectName: "Global",
      enabled: hasGlobalPlacement(skill.cells),
      defaultAgent: globalTargets[0] ?? defaultGlobalEntry,
      targetAgents: globalTargets,
    },
  ];

  for (const project of projects) {
    const incoming = allSkills.find(
      (s) =>
        s.canonicalSkillId === skill.id &&
        s.placementScope === "project" &&
        s.placementProjectId === project.id,
    );
    const projectTargets = incoming ? targetAgentsOf(incoming.cells) : [];
    targets.push({
      kind: "project",
      projectId: project.id,
      projectName: project.name,
      enabled: projectTargets.length > 0,
      defaultAgent: projectTargets[0] ?? defaultGlobalEntry,
      targetAgents: projectTargets,
    });
  }

  return targets;
}
