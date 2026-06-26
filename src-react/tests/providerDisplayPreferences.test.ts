import test from "node:test";
import assert from "node:assert/strict";

function applyCardVisibility(
  providerIds: readonly string[],
  savedVisible: readonly string[] | undefined,
  hiddenByDefault: ReadonlySet<string>,
): Record<string, boolean> {
  const saved = new Set(savedVisible ?? []);
  const hasSaved = saved.size > 0;
  return Object.fromEntries(
    providerIds.map((providerId) => [
      providerId,
      hasSaved ? saved.has(providerId) : !hiddenByDefault.has(providerId),
    ]),
  );
}

function nextCardVisibility(
  order: readonly string[],
  cardVisible: Readonly<Record<string, boolean>>,
  providerId: string,
  visible: boolean,
) {
  const next = { ...cardVisible, [providerId]: visible };
  return {
    next,
    cardVisibility: order.filter((id) => next[id] !== false),
  };
}

test("saved provider card visibility overrides catalog defaults after reload", () => {
  const providerIds = ["claude", "copilot", "opencode-go"];
  const hiddenByDefault = new Set<string>(["opencode-go"]);

  assert.deepEqual(applyCardVisibility(providerIds, undefined, hiddenByDefault), {
    claude: true,
    copilot: true,
    "opencode-go": false,
  });

  assert.deepEqual(applyCardVisibility(providerIds, ["copilot"], hiddenByDefault), {
    claude: false,
    copilot: true,
    "opencode-go": false,
  });
});

test("persisted card visibility payload follows current provider order", () => {
  const order = ["copilot", "claude", "opencode-go"];
  const current = {
    claude: true,
    copilot: true,
    "opencode-go": false,
  };

  assert.deepEqual(nextCardVisibility(order, current, "claude", false), {
    next: {
      claude: false,
      copilot: true,
      "opencode-go": false,
    },
    cardVisibility: ["copilot"],
  });

  assert.deepEqual(nextCardVisibility(order, current, "opencode-go", true), {
    next: {
      claude: true,
      copilot: true,
      "opencode-go": true,
    },
    cardVisibility: ["copilot", "claude", "opencode-go"],
  });
});
