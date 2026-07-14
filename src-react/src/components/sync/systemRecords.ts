import type { SessionBackup, TaskGroup } from "../../types/index.js";

export function sessionBackupsToTaskGroup(backups: SessionBackup[]): TaskGroup {
  return {
    id: "system:session-backup",
    name: "Session Backup",
    collapsed: false,
    tasks: backups.map(({ task }) => task),
  };
}
