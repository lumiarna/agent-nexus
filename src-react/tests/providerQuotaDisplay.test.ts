import test from "node:test";
import assert from "node:assert/strict";

import {
  formatProviderQuotaDisplay,
  isQuotaPaceAlert,
} from "../src/components/provider/quotaDisplay.js";

test("Claude Code quota display keeps the current card's used-percent semantics", () => {
  const provider = {
    id: "claude",
    name: "Claude Code",
    plan: "Claude Pro",
    status: "available",
    credential: "~/.claude",
    primary: 0,
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
    primaryLabel: "0%",
    primaryCaption: "shortest window used",
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

  assert.deepEqual(
    formatProviderQuotaDisplay(provider, {
      timeZone: "Asia/Shanghai",
      now: new Date("2026-06-16T00:00:00Z"),
    }),
    {
      primaryLabel: "77%",
      primaryCaption: "shortest window used",
      windows: [
        { label: "Premium Interactions", usedLabel: "77%", used: 77, reset: "Resets Jul 1", unlimited: false, pace: 50 },
        { label: "Chat Quota", usedLabel: "20%", used: 20, reset: "Resets Jul 1", unlimited: false, pace: 50 },
      ],
    },
  );
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

  assert.deepEqual(formatProviderQuotaDisplay(provider, { now: new Date("2026-06-16T00:00:00Z") }), {
    primaryLabel: "100%",
    primaryCaption: "shortest window used",
    windows: [
      { label: "Premium Interactions", usedLabel: "100%", used: 100, reset: "Resets Jul 1", unlimited: false, pace: 50 },
      { label: "Chat Quota", usedLabel: "Unlimited", used: 0, reset: "Resets Jul 1", unlimited: true },
    ],
  });
});

test("Codex reset credits render expiry in local 24-hour time to the minute", () => {
  const provider = {
    primary: null,
    windows: [
      {
        label: "Full reset (Weekly + 5 hr)",
        used: 0,
        kind: "rolling",
        valueLabel: "Available",
        valueOnly: true,
        resetAt: "2026-07-15T19:22:24.080059Z",
      },
    ],
  };

  assert.deepEqual(formatProviderQuotaDisplay(provider, { timeZone: "Asia/Shanghai" }), {
    primaryLabel: "",
    primaryCaption: "",
    windows: [
      {
        label: "Full reset (Weekly + 5 hr)",
        usedLabel: "Expires Jul 16 03:22",
        used: 0,
        reset: "",
        unlimited: false,
        valueOnly: true,
      },
    ],
  });
});

test("balance-only windows render the backend value label without a primary percent", () => {
  const provider = {
    primary: null,
    windows: [
      {
        label: "CNY balance",
        used: 0,
        kind: "monthly",
        valueLabel: "12.34 CNY",
        valueOnly: true,
      },
    ],
  };

  assert.deepEqual(formatProviderQuotaDisplay(provider), {
    primaryLabel: "",
    primaryCaption: "",
    windows: [
      {
        label: "CNY balance",
        usedLabel: "12.34 CNY",
        used: 0,
        reset: "",
        unlimited: false,
        valueOnly: true,
      },
    ],
  });
});

test("Qoder monthly windows keep numeric value labels and pace markers", () => {
  const provider = {
    primary: null,
    windows: [
      {
        label: "Monthly limit",
        used: 1,
        kind: "monthly",
        valueLabel: "17 / 3,000 credits",
        resetAt: "2026-08-01T00:00:00Z",
      },
    ],
  };

  assert.deepEqual(
    formatProviderQuotaDisplay(provider, { now: new Date("2026-07-16T12:00:00Z") }),
    {
      primaryLabel: "",
      primaryCaption: "",
      windows: [
        {
          label: "Monthly limit",
          usedLabel: "17 / 3,000 credits",
          used: 1,
          reset: "Resets Aug 1",
          unlimited: false,
          pace: 50,
        },
      ],
    },
  );
});

test("pace is derived only for weekly/monthly windows that carry a resetAt", () => {
  const provider = {
    primary: null,
    windows: [
      // 3.5 days into a 7-day window → marker at the midpoint.
      { label: "Weekly limit", used: 80, kind: "weekly", resetAt: "2026-06-21T07:00:00Z" },
      // Short rolling window: excluded even with a resetAt (uniform burn doesn't hold).
      { label: "5-hour limit", used: 80, kind: "rolling", resetAt: "2026-06-17T22:00:00Z" },
      // Monthly without a resetAt: no interval to place a marker on.
      { label: "Monthly limit", used: 10, kind: "monthly" },
    ],
  };

  const display = formatProviderQuotaDisplay(provider, {
    now: new Date("2026-06-17T19:00:00Z"),
  });

  assert.equal(display.windows[0].pace, 50);
  assert.equal(display.windows[1].pace, undefined);
  assert.equal(display.windows[2].pace, undefined);
});

test("natural-month quota warns only when usage is ahead of the long-window pace", () => {
  const display = formatProviderQuotaDisplay(
    {
      windows: [
        {
          label: "Monthly limit",
          used: 56,
          kind: "monthly",
          resetAt: "2026-07-01T00:00:00Z",
        },
        {
          label: "Daily limit",
          used: 99,
          kind: "rolling",
          resetAt: "2026-06-07T00:00:00Z",
        },
      ],
    },
    { now: new Date("2026-06-06T00:00:00Z") },
  );

  assert.equal(isQuotaPaceAlert(display.windows[0]), true);
  assert.equal(display.windows[1].pace, undefined);
  assert.equal(isQuotaPaceAlert(display.windows[1]), false);
});
