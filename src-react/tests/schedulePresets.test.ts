import test from "node:test";
import assert from "node:assert/strict";

import { DEFAULT_CRON_SCHEDULE, SCHEDULE_PRESETS } from "../src/components/sync/schedule.js";

test("sync schedule presets default to five minutes, hourly, and daily", () => {
  assert.equal(DEFAULT_CRON_SCHEDULE, "*/5 * * * *");
  assert.deepEqual(
    SCHEDULE_PRESETS.map((preset) => preset.expr),
    ["*/5 * * * *", "0 * * * *", "0 5 * * *"],
  );
});
