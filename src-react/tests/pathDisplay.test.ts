import test from "node:test";
import assert from "node:assert/strict";

import { formatProjectSymlinkDisplayPath } from "../src/components/sync/pathDisplay.js";

test("external Windows verbatim paths are displayed without the question-mark prefix", () => {
  const fullPath = String.raw`\\?\D:\Workspace\ros\ros_backend\src\main`;

  assert.equal(
    formatProjectSymlinkDisplayPath(fullPath, null),
    String.raw`D:\Workspace\ros\ros_backend\src\main`,
  );
});

test("registered project paths are displayed relative to the project root on Windows", () => {
  assert.equal(
    formatProjectSymlinkDisplayPath(
      String.raw`D:\Vault\TMS\.agents\skills\skill-creator`,
      "TMS",
    ),
    ".agents/skills/skill-creator",
  );
});

test("registered project paths are displayed relative to the project root with slash separators", () => {
  assert.equal(
    formatProjectSymlinkDisplayPath("D:/Vault/TMS/.claude/skills/skill-creator", "TMS"),
    ".claude/skills/skill-creator",
  );
});
