import test from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_QUOTA_REFRESH_MINUTES,
  QUOTA_REFRESH_PRESETS,
  WINDOW_ALIGN_CRON_PRESETS,
  isWindowAlignActive,
  quotaRefreshIntervalMs,
  windowAlignCronHuman,
  windowAlignLastAttemptLabel,
  windowAlignStatusLabel,
} from "../src/components/provider/providerSchedule.js";

test("quota refresh defaults to five minutes with interval presets", () => {
  assert.equal(DEFAULT_QUOTA_REFRESH_MINUTES, 5);
  assert.deepEqual(
    QUOTA_REFRESH_PRESETS.map((preset) => preset.minutes),
    [1, 5, 15, 30, 60],
  );
});

test("quota refresh minutes convert to react-query milliseconds", () => {
  assert.equal(quotaRefreshIntervalMs(5), 300_000);
  assert.equal(quotaRefreshIntervalMs(15), 900_000);
  // invalid / empty falls back to the default cadence, never zero
  assert.equal(quotaRefreshIntervalMs(null), 300_000);
  assert.equal(quotaRefreshIntervalMs(0), 60_000);
});

test("window alignment presets lead with the 05/10/15/20 schedule", () => {
  assert.equal(WINDOW_ALIGN_CRON_PRESETS[0].expr, "0 5,10,15,20 * * *");
});

test("window alignment cron humanizes the minute-hours daily shape", () => {
  assert.equal(
    windowAlignCronHuman("0 5,10,15,20 * * *"),
    "Every day at 05:00 · 10:00 · 15:00 · 20:00.",
  );
  assert.equal(windowAlignCronHuman("0 9 * * *"), "Every day at 09:00.");
});

test("window alignment cron describes empty and custom expressions honestly", () => {
  assert.equal(
    windowAlignCronHuman("   "),
    "Add a time and model to enable window alignment.",
  );
  assert.equal(windowAlignCronHuman("*/5 * * * *"), "Custom schedule expression.");
  assert.equal(windowAlignCronHuman("0 99 * * *"), "Custom schedule expression.");
});

test("window alignment requires both a cron and a model to be active", () => {
  assert.equal(isWindowAlignActive("0 9 * * *", "claude-haiku-4-5-20251001"), true);
  assert.equal(isWindowAlignActive("", "claude-haiku-4-5-20251001"), false);
  assert.equal(isWindowAlignActive("0 9 * * *", ""), false);
  assert.equal(isWindowAlignActive("0 9 * * *", null), false);
});

test("window alignment last run labels show empty and known statuses", () => {
  assert.equal(windowAlignLastAttemptLabel(null), "Never triggered");
  assert.match(windowAlignLastAttemptLabel(1_787_936_400), /\d{2}/);
  assert.equal(windowAlignStatusLabel("success"), "Success");
  assert.equal(windowAlignStatusLabel("retryable_failed"), "Temporary failure");
  assert.equal(windowAlignStatusLabel("terminal_failed"), "Failed");
  assert.equal(windowAlignStatusLabel("never"), "No result yet");
});
