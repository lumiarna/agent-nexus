import test from "node:test";
import assert from "node:assert/strict";

import { builtInProviderRows } from "../src/lib/providerCatalog.js";

test("configured provider catalog includes MiniMax Token Plan CN after OpenCode Go and Qoder", () => {
  const visibleProviders = builtInProviderRows().filter((provider) => !provider.hiddenCard);

  assert.deepEqual(
    visibleProviders.slice(0, 5).map((provider) => provider.id),
    ["opencode-go", "qoder", "minimax-token", "deepseek", "openrouter"],
  );
  assert.equal(visibleProviders[2].name, "MiniMax Token Plan CN");
});
