import test from "node:test";
import assert from "node:assert/strict";

import { providerRowsFromAgentCapabilities } from "../src/lib/agentCapabilities.js";
import type { AgentCapabilitySurface } from "../src/lib/api/agentCapabilities.js";
import type { Provider } from "../src/types/index.js";

const capabilities = [
  {
    name: "Generic Agent",
    abbr: "AG",
    color: "#9a7b53",
    configDir: "~/.agents",
    skill: null,
    prompt: null,
    provider: null,
  },
  {
    name: "Claude Code",
    abbr: "CC",
    color: "#c2410c",
    configDir: "~/.claude",
    skill: null,
    prompt: null,
    provider: { providerId: "claude", credentialHint: "~/.claude" },
  },
  {
    name: "CodeX",
    abbr: "CX",
    color: "#4f7a6a",
    configDir: "~/.codex",
    skill: null,
    prompt: null,
    provider: { providerId: "codex", credentialHint: "~/.codex/auth.json" },
  },
  {
    name: "Copilot",
    abbr: "CP",
    color: "#5a7894",
    configDir: "~/.github",
    skill: null,
    prompt: null,
    provider: { providerId: "copilot", credentialHint: "$GITHUB_TOKEN" },
  },
] satisfies AgentCapabilitySurface[];

test("agent-backed provider rows derive identity and credentials from capability surfaces", () => {
  const existingProviders = [
    {
      id: "claude",
      name: "Old Claude label",
      plan: "Claude Pro",
      status: "available",
      credential: "runtime credential",
      isAgent: false,
    },
    {
      id: "opencode-go",
      name: "OpenCode Go",
      plan: "Workspace",
      status: "nocreds",
      credential: "manual params",
      needsParams: true,
    },
  ] satisfies Provider[];

  const rows = providerRowsFromAgentCapabilities(capabilities, existingProviders);

  assert.deepEqual(
    rows.map((provider) => ({
      id: provider.id,
      name: provider.name,
      credential: provider.credential,
      isAgent: provider.isAgent ?? false,
      needsParams: provider.needsParams ?? false,
    })),
    [
      {
        id: "claude",
        name: "Claude Code",
        credential: "~/.claude",
        isAgent: true,
        needsParams: false,
      },
      {
        id: "codex",
        name: "CodeX",
        credential: "~/.codex/auth.json",
        isAgent: true,
        needsParams: false,
      },
      {
        id: "copilot",
        name: "Copilot",
        credential: "$GITHUB_TOKEN",
        isAgent: true,
        needsParams: false,
      },
      {
        id: "opencode-go",
        name: "OpenCode Go",
        credential: "manual params",
        isAgent: false,
        needsParams: true,
      },
    ],
  );
  assert.equal(rows.some((provider) => provider.name === "Generic Agent"), false);
});
