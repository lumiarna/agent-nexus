import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { syncApi } from "@/lib/api/sync";
import type { AddTaskInput, CreateTaskGroupInput } from "@/lib/api/sync";
import type { Task, TaskGroup } from "@/types";

export const syncKeys = {
  webdavSettings: ["sync", "webdavSettings"] as const,
  taskGroups: ["sync", "taskGroups"] as const,
};

export function useWebdavSettingsQuery() {
  return useQuery({
    queryKey: syncKeys.webdavSettings,
    queryFn: syncApi.getWebdavSettings,
  });
}

export function useSaveWebdavSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: syncApi.saveWebdavSettings,
    onSuccess: (settings) => {
      queryClient.setQueryData(syncKeys.webdavSettings, settings);
    },
  });
}

export function useTestWebdavConnectionMutation() {
  return useMutation({
    mutationFn: syncApi.testWebdavConnection,
  });
}

export function useTaskGroupsQuery() {
  return useQuery({
    queryKey: syncKeys.taskGroups,
    queryFn: syncApi.listTaskGroups,
  });
}

export function useCreateTaskGroupMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateTaskGroupInput) => syncApi.createTaskGroup(input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.taskGroups }),
  });
}

export function useDeleteTaskMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => syncApi.deleteTask(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.taskGroups }),
  });
}

export function useDeleteTaskGroupMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => syncApi.deleteTaskGroup(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.taskGroups }),
  });
}

export function useAddTaskMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ groupId, task }: { groupId: string; task: AddTaskInput }) =>
      syncApi.addTask(groupId, task),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.taskGroups }),
  });
}

export function useUpdateTaskScheduleMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, schedule }: { id: string; schedule: string }) =>
      syncApi.updateTaskSchedule(id, schedule),
    onSuccess: (updated) => {
      queryClient.setQueryData<TaskGroup[]>(syncKeys.taskGroups, (groups) =>
        replaceTask(groups, updated),
      );
    },
  });
}

export function useRunTaskMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => syncApi.runTask(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.taskGroups }),
  });
}

function replaceTask(groups: TaskGroup[] | undefined, updated: Task): TaskGroup[] | undefined {
  return groups?.map((group) => ({
    ...group,
    tasks: group.tasks.map((task) => (task.id === updated.id ? updated : task)),
  }));
}
