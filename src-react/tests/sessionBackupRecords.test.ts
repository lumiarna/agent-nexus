import test from "node:test";
import assert from "node:assert/strict";

import { sessionBackupsToTaskGroup } from "../src/components/sync/systemRecords.js";

test("maps backend Session Backup tasks into a system-managed Task Group", () => {
  const task = {
    id: "session-backup:project-1",
    direction: "Push" as const,
    action: "Copy" as const,
    sourceType: "Local" as const,
    source: "/workspace/agent-nexus/.sessions/",
    targetType: "Cloud" as const,
    target: "Session/agent-nexus/",
    schedule: "0 * * * *",
    lastRunAt: 1750000000,
    status: "ok" as const,
    linkState: "present" as const,
  };
  const group = sessionBackupsToTaskGroup([
    {
      projectKey: "agent-nexus",
      task,
    },
  ]);

  assert.deepEqual(group, {
    id: "system:session-backup",
    name: "Session Backup",
    collapsed: false,
    tasks: [task],
  });
});
