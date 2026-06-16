import type { ProjectSymlink, TaskAction, TaskGroup, LocationType } from "@/types";
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

export const syncApi = {
  listTaskGroups(): Promise<TaskGroup[]> {
    return invokeCommand<TaskGroup[]>("list_task_groups");
  },
  createTaskGroup(input: CreateTaskGroupInput): Promise<TaskGroup> {
    return invokeCommand<TaskGroup>("create_task_group", { input });
  },
  deleteTask(id: string): Promise<void> {
    return invokeCommand<void>("delete_task", { id });
  },
  listProjectSymlinks(): Promise<ProjectSymlink[]> {
    return invokeCommand<ProjectSymlink[]>("list_project_symlinks");
  },
  deleteProjectSymlink(targetPath: string): Promise<void> {
    return invokeCommand<void>("delete_project_symlink", { targetPath });
  },
};
