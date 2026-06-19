import test from "node:test";
import assert from "node:assert/strict";

import { formatProviderQuotaDisplay } from "../src/components/provider/quotaDisplay.js";

test("Claude Code quota display keeps the current card's used-percent semantics", () => {
  const provider = {
    id: "claude",
    name: "Claude Code",
    plan: "Claude Pro",
    status: "available",
    credential: "~/.claude",
    primary: 59,
    windows: [
      { label: "5-hour limit", used: 0, reset: "Resets in 4h 59m" },
      {
        label: "Weekly limit",
        used: 59,
        reset: "Resets Mon 09:00",
        resetAt: "2026-06-21T07:00:00Z",
      },
    ],
  };

  assert.deepEqual(formatProviderQuotaDisplay(provider, { timeZone: "Asia/Shanghai" }), {
    primaryLabel: "59%",
    primaryCaption: "peak window used",
    windows: [
      { label: "5-hour limit", usedLabel: "0%", used: 0, reset: "Resets in 4h 59m", unlimited: false },
      { label: "Weekly limit", usedLabel: "59%", used: 59, reset: "Resets Sun 15:00", unlimited: false },
    ],
  });
});

test("Copilot monthly windows render the calendar reset date", () => {
  const provider = {
    primary: 77,
    windows: [
      {
        label: "Premium Interactions",
        used: 77,
        kind: "monthly",
        resetAt: "2026-07-01T00:00:00Z",
      },
      {
        label: "Chat Quota",
        used: 20,
        kind: "monthly",
        resetAt: "2026-07-01T00:00:00Z",
      },
    ],
  };

  assert.deepEqual(formatProviderQuotaDisplay(provider, { timeZone: "Asia/Shanghai" }), {
    primaryLabel: "77%",
    primaryCaption: "peak window used",
    windows: [
      { label: "Premium Interactions", usedLabel: "77%", used: 77, reset: "Resets Jul 1", unlimited: false },
      { label: "Chat Quota", usedLabel: "20%", used: 20, reset: "Resets Jul 1", unlimited: false },
    ],
  });
});

test("Copilot unlimited window renders an Unlimited label", () => {
  const provider = {
    primary: 100,
    windows: [
      {
        label: "Premium Interactions",
        used: 100,
        kind: "monthly",
        resetAt: "2026-07-01T00:00:00Z",
      },
      {
        label: "Chat Quota",
        used: 0,
        kind: "monthly",
        resetAt: "2026-07-01T00:00:00Z",
        unlimited: true,
      },
    ],
  };

  assert.deepEqual(formatProviderQuotaDisplay(provider), {
    primaryLabel: "100%",
    primaryCaption: "peak window used",
    windows: [
      { label: "Premium Interactions", usedLabel: "100%", used: 100, reset: "Resets Jul 1", unlimited: false },
      { label: "Chat Quota", usedLabel: "Unlimited", used: 0, reset: "Resets Jul 1", unlimited: true },
    ],
  });
});
