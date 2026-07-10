import test from "node:test";
import assert from "node:assert/strict";

import { supportsWindowAlignment } from "../src/components/provider/windowAlignmentSupport.js";

test("window alignment is enabled for Claude Code and CodeX only", () => {
  assert.equal(supportsWindowAlignment("claude"), true);
  assert.equal(supportsWindowAlignment("codex"), true);
  assert.equal(supportsWindowAlignment("copilot"), false);
  assert.equal(supportsWindowAlignment("opencode-go"), false);
  assert.equal(supportsWindowAlignment(null), false);
});
