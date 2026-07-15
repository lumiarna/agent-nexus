import test from "node:test";
import assert from "node:assert/strict";

import { projectForSkillRow, showsInGlobalSkillPage } from "../src/components/skill/visibility.js";
import type { PlacementCellRole, Skill } from "../src/types/index.js";

// Contract guard: Project custom cells cannot represent an Agent source.
// @ts-expect-error `source` belongs only to AgentCellRole.
const invalidPlacementRole: PlacementCellRole = "source";
void invalidPlacementRole;

const placementCells = {
  "Generic Agent": "none",
  "Claude Code": "none",
  CodeX: "none",
  Copilot: "none",
  OpenCode: "none",
  Pi: "none",
  Qoder: "none",
} as const;

const summary = {
  skillId: "skill-1",
  name: "Demo",
  desc: "Demo Skill",
  path: "/demo",
  disabled: false,
};

test("explicit Skill row variants drive visibility and project identity", () => {
  const global: Skill = {
    kind: "agentCanonical",
    rowKey: "global",
    skill: summary,
    context: { kind: "global" },
    sourceAgent: "Generic Agent",
    cells: { ...placementCells, "Generic Agent": "source" },
  };
  assert.equal(showsInGlobalSkillPage(global), true);
  assert.equal(projectForSkillRow(global), undefined);

  const canonical: Skill = {
    kind: "projectCustomCanonical",
    rowKey: "custom",
    skill: summary,
    sourceProject: { id: "source", name: "Source" },
    destinations: [
      { kind: "global", cells: { ...placementCells, "Claude Code": "target" } },
      {
        kind: "project",
        project: { id: "target", name: "Target" },
        cells: placementCells,
      },
    ],
  };
  assert.equal(showsInGlobalSkillPage(canonical), true);
  assert.deepEqual(projectForSkillRow(canonical), canonical.sourceProject);

  const incoming: Skill = {
    kind: "projectCustomIncoming",
    rowKey: "incoming",
    skill: summary,
    sourceProject: { id: "source", name: "Source" },
    targetProject: { id: "target", name: "Target" },
    cells: { ...placementCells, CodeX: "target" },
  };
  assert.equal(showsInGlobalSkillPage(incoming), false);
  assert.deepEqual(projectForSkillRow(incoming), incoming.targetProject);
});

test("Project custom canonical without a Global placement stays hidden", () => {
  const row: Skill = {
    kind: "projectCustomCanonical",
    rowKey: "custom",
    skill: summary,
    sourceProject: { id: "source", name: "Source" },
    destinations: [{ kind: "global", cells: placementCells }],
  };
  assert.equal(showsInGlobalSkillPage(row), false);
});
