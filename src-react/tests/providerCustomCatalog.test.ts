import test from "node:test";
import assert from "node:assert/strict";

import { customProviderRows } from "../src/lib/providerCatalog.js";

test("OpenCode custom providers become cards without overriding built-in providers", () => {
  const rows = customProviderRows(
    [
      {
        id: "llm-gateway-alicloud",
        name: "LLM Gateway AliCloud",
        npm: "@ai-sdk/openai-compatible",
        baseUrl: "https://gateway.example/deployments/ali",
        modelId: "deepseek-v4-flash",
      },
      {
        id: "codex",
        name: "Must not replace CodeX",
        npm: "@ai-sdk/openai",
        baseUrl: "https://gateway.example/v1",
        modelId: "gpt-5.4",
      },
    ],
    [{ id: "codex", name: "CodeX", plan: "Plus", status: "available" }],
  );

  assert.deepEqual(rows, [
    {
      id: "llm-gateway-alicloud",
      name: "LLM Gateway AliCloud",
      plan: "OpenCode custom",
      status: "nocreds",
      credential: "opencode.json · llm-gateway-alicloud",
    },
  ]);
});
