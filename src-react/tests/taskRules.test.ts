import test from "node:test";
import assert from "node:assert/strict";

import {
  actionOptions,
  expandFormTask,
  hasCloudEndpoint,
  isCronSchedule,
  normalizeSchedule,
  scheduleForAction,
  scheduleForMode,
} from "../src/components/sync/taskRules.js";

test("hasCloudEndpoint is true when source or any target is Cloud", () => {
  assert.equal(hasCloudEndpoint({ sourceType: "Local", targets: [{ type: "Local" }] }), false);
  assert.equal(hasCloudEndpoint({ sourceType: "Cloud", targets: [{ type: "Local" }] }), true);
  assert.equal(
    hasCloudEndpoint({ sourceType: "Local", targets: [{ type: "Local" }, { type: "Cloud" }] }),
    true,
  );
});

test("actionOptions disables Symlink/Junction when an endpoint is Cloud", () => {
  const localOnly = { sourceType: "Local" as const, targets: [{ type: "Local" as const }] };
  assert.deepEqual(actionOptions(localOnly, true), [
    { value: "Symlink", label: "Symlink", disabled: false },
    { value: "Junction", label: "Junction", disabled: false },
    { value: "Copy", label: "Copy" },
  ]);

  const withCloud = { sourceType: "Local" as const, targets: [{ type: "Cloud" as const }] };
  assert.deepEqual(actionOptions(withCloud, true), [
    { value: "Symlink", label: "Symlink", disabled: true },
    { value: "Junction", label: "Junction", disabled: true },
    { value: "Copy", label: "Copy" },
  ]);
});

test("actionOptions hides Junction when the platform does not support it", () => {
  const localOnly = { sourceType: "Local" as const, targets: [{ type: "Local" as const }] };
  assert.deepEqual(
    actionOptions(localOnly, false).map((option) => option.value),
    ["Symlink", "Copy"],
  );
});

test("scheduleForAction keeps a schedule only for Copy", () => {
  assert.equal(scheduleForAction("Copy", "0 5 * * *"), "0 5 * * *");
  assert.equal(scheduleForAction("Symlink", "0 5 * * *"), "manual");
  assert.equal(scheduleForAction("Junction", "0 5 * * *"), "manual");
});

test("isCronSchedule treats only 'manual' as non-cron", () => {
  assert.equal(isCronSchedule("manual"), false);
  assert.equal(isCronSchedule("0 5 * * *"), true);
});

test("scheduleForMode toggles manual/cron and preserves an existing cron", () => {
  assert.equal(scheduleForMode("manual", "0 5 * * *", "*/5 * * * *"), "manual");
  assert.equal(scheduleForMode("cron", "0 5 * * *", "*/5 * * * *"), "0 5 * * *");
  assert.equal(scheduleForMode("cron", "manual", "*/5 * * * *"), "*/5 * * * *");
});

test("normalizeSchedule collapses blanks to manual and trims cron", () => {
  assert.equal(normalizeSchedule(""), "manual");
  assert.equal(normalizeSchedule("   "), "manual");
  assert.equal(normalizeSchedule(" 0 5 * * * "), "0 5 * * *");
});

test("expandFormTask drops blank targets and maps each to a single-target draft", () => {
  const drafts = expandFormTask({
    action: "Copy",
    sourceType: "Local",
    source: "~/.config/warp/",
    targets: [
      { type: "Cloud", path: " backups/warp/ " },
      { type: "Local", path: "   " },
      { type: "Local", path: "/mirror/warp/" },
    ],
    schedule: "  ",
  });

  assert.deepEqual(drafts, [
    {
      action: "Copy",
      sourceType: "Local",
      source: "~/.config/warp/",
      targetType: "Cloud",
      target: "backups/warp/",
      schedule: "manual",
    },
    {
      action: "Copy",
      sourceType: "Local",
      source: "~/.config/warp/",
      targetType: "Local",
      target: "/mirror/warp/",
      schedule: "manual",
    },
  ]);
});
