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
      { label: "5-hour limit", usedLabel: "0%", used: 0, reset: "Resets in 4h 59m" },
      { label: "Weekly limit", usedLabel: "59%", used: 59, reset: "Resets Sun 15:00" },
    ],
  });
});
