import test from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_QUOTA_REFRESH_MINUTES,
  QUOTA_REFRESH_PRESETS,
  WINDOW_ALIGN_START_TIME_PRESETS,
  isWindowAlignActive,
  quotaRefreshIntervalMs,
  windowAlignCronToStartTime,
  windowAlignLastAttemptLabel,
  windowAlignStartTimeHuman,
  windowAlignStartTimeToCron,
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

test("window alignment presets expose a single local first trigger time", () => {
  assert.deepEqual(
    WINDOW_ALIGN_START_TIME_PRESETS.map((preset) => preset.value),
    ["01:00", "02:00", "03:00", "04:00", "05:00"],
  );
});

test("window alignment converts between local first trigger time and daily cron", () => {
  assert.equal(windowAlignStartTimeToCron("05:00"), "0 5 * * *");
  assert.equal(windowAlignStartTimeToCron("8:30"), "30 8 * * *");
  assert.equal(windowAlignCronToStartTime("30 8 * * *"), "08:30");
  assert.equal(windowAlignCronToStartTime("0 5,10,15,20 * * *"), "05:00");
});

test("window alignment start time describes empty and valid values", () => {
  assert.equal(
    windowAlignStartTimeHuman(""),
    "Add a local first trigger time and model to enable window alignment.",
  );
  assert.equal(
    windowAlignStartTimeHuman("05:00"),
    "Every day starts at 05:00 local time; later attempts follow the 5-hour window.",
  );
});

test("window alignment requires both a start time and a model to be active", () => {
  assert.equal(isWindowAlignActive("09:00", "claude-haiku-4-5-20251001"), true);
  assert.equal(isWindowAlignActive("", "claude-haiku-4-5-20251001"), false);
  assert.equal(isWindowAlignActive("09:00", ""), false);
  assert.equal(isWindowAlignActive("09:00", null), false);
  assert.equal(isWindowAlignActive("25:00", "claude-haiku-4-5-20251001"), false);
});

test("window alignment last run labels show empty and known statuses", () => {
  assert.equal(windowAlignLastAttemptLabel(null), "Never triggered");
  assert.match(windowAlignLastAttemptLabel(1_787_936_400), /\d{2}/);
  assert.equal(windowAlignStatusLabel("success"), "Success");
  assert.equal(windowAlignStatusLabel("retryable_failed"), "Temporary failure");
  assert.equal(windowAlignStatusLabel("terminal_failed"), "Failed");
  assert.equal(windowAlignStatusLabel("never"), "No result yet");
});
