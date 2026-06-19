import type { TaskAction, TaskGroup, LocationType, WebdavSettings } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export interface CreateTaskInput {
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targetType: LocationType;
  target: string;
  schedule: string;
}

export interface CreateTaskGroupInput {
  name: string;
  tasks: CreateTaskInput[];
}

export interface AddTaskInput {
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targetType: LocationType;
  target: string;
  schedule: string;
}

export interface SaveWebdavSettingsInput {
  url: string;
  user: string;
  pass: string;
  remoteRoot: string;
}

export const syncApi = {
  getWebdavSettings(): Promise<WebdavSettings> {
    return invokeCommand<WebdavSettings>("get_webdav_settings");
  },
  saveWebdavSettings(input: SaveWebdavSettingsInput): Promise<WebdavSettings> {
    return invokeCommand<WebdavSettings>("save_webdav_settings", { input });
  },
  testWebdavConnection(input: SaveWebdavSettingsInput): Promise<void> {
    return invokeCommand<void>("test_webdav_connection", { input });
  },
  listTaskGroups(): Promise<TaskGroup[]> {
    return invokeCommand<TaskGroup[]>("list_task_groups");
  },
  createTaskGroup(input: CreateTaskGroupInput): Promise<TaskGroup> {
    return invokeCommand<TaskGroup>("create_task_group", { input });
  },
  deleteTask(id: string): Promise<void> {
    return invokeCommand<void>("delete_task", { id });
  },
  deleteTaskGroup(id: string): Promise<void> {
    return invokeCommand<void>("delete_task_group", { id });
  },
  addTask(groupId: string, task: AddTaskInput): Promise<TaskGroup> {
    return invokeCommand<TaskGroup>("add_task", { groupId, task });
  },
  runTask(id: string): Promise<TaskGroup["tasks"][number]> {
    return invokeCommand<TaskGroup["tasks"][number]>("run_task", { id });
  },
};
