import test from "node:test";
import assert from "node:assert/strict";

import { showsInGlobalSkillPage } from "../src/components/skill/visibility.js";

const baseCells = {
  "Generic Agent": "none",
  "Claude Code": "none",
  CodeX: "none",
};

test("global canonical skills stay visible on the Global page", () => {
  assert.equal(
    showsInGlobalSkillPage({
      scope: "global",
      sourceKind: "agent",
      cells: baseCells,
    }),
    true,
  );
});

test("canonical project custom source rows with a Global placement stay visible", () => {
  assert.equal(
    showsInGlobalSkillPage({
      scope: "project",
      sourceKind: "project_custom",
      cells: { ...baseCells, "Claude Code": "target" },
    }),
    true,
  );
});

test("project placement projection rows stay out of the Global page", () => {
  assert.equal(
    showsInGlobalSkillPage({
      scope: "project",
      sourceKind: "project_custom",
      placementScope: "project",
      cells: { ...baseCells, "Claude Code": "target" },
    }),
    false,
  );
});

test("project custom source rows without any Global placement stay hidden", () => {
  assert.equal(
    showsInGlobalSkillPage({
      scope: "project",
      sourceKind: "project_custom",
      cells: baseCells,
    }),
    false,
  );
});
