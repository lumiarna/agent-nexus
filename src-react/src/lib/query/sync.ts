import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { syncApi } from "@/lib/api/sync";
import type { CreateTaskGroupInput } from "@/lib/api/sync";

export const syncKeys = {
  taskGroups: ["sync", "taskGroups"] as const,
  projectSymlinks: ["sync", "projectSymlinks"] as const,
};

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

export function useProjectSymlinksQuery() {
  return useQuery({
    queryKey: syncKeys.projectSymlinks,
    queryFn: syncApi.listProjectSymlinks,
  });
}

export function useDeleteProjectSymlinkMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (targetPath: string) => syncApi.deleteProjectSymlink(targetPath),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncKeys.projectSymlinks }),
  });
}
