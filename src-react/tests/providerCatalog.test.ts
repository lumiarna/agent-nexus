import test from "node:test";
import assert from "node:assert/strict";

import { nexus } from "../src/lib/mock.js";

test("MiniMax Token Plan CN is the default fifth visible provider card", () => {
  const visibleProviders = nexus.providers().filter((provider) => !provider.hiddenCard);

  assert.deepEqual(
    visibleProviders.slice(0, 5).map((provider) => provider.id),
    ["claude", "codex", "copilot", "opencode-go", "minimax-token"],
  );
  assert.equal(visibleProviders[4].name, "MiniMax Token Plan CN");
});
