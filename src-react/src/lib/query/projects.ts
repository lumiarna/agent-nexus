import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { projectsApi } from "@/lib/api/projects";
import { projectSymlinkKeys } from "@/lib/query/projectSymlinkInventory";
import { sessionKeys } from "@/lib/query/sessions";
import { skillKeys } from "@/lib/query/skills";
import type { Project } from "@/types";

export const projectKeys = {
  all: ["projects"] as const,
  gitBaseFolders: ["projects", "gitBaseFolders"] as const,
};

export function useProjectsQuery() {
  return useQuery({
    queryKey: projectKeys.all,
    queryFn: projectsApi.list,
  });
}

export function useRecordProjectMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: projectsApi.record,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useRecordProjectsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (paths: string[]) => Promise.all(paths.map((path) => projectsApi.record(path))),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useReorderProjectsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (projectIds: string[]) => projectsApi.reorder(projectIds),
    onSuccess: async (projects: Project[]) => {
      queryClient.setQueryData<Project[]>(projectKeys.all, projects);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: skillKeys.all }),
        queryClient.invalidateQueries({ queryKey: sessionKeys.local }),
        queryClient.invalidateQueries({ queryKey: projectSymlinkKeys.inventory }),
      ]);
    },
  });
}

export function useGitBaseFoldersQuery() {
  return useQuery({
    queryKey: projectKeys.gitBaseFolders,
    queryFn: projectsApi.listGitBaseFolders,
  });
}

export function useRecordGitBaseFolderMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: projectsApi.recordGitBaseFolder,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: projectKeys.gitBaseFolders });
    },
  });
}

export function useRemoveGitBaseFolderMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: projectsApi.removeGitBaseFolder,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: projectKeys.gitBaseFolders });
    },
  });
}

export function useScanGitBaseFoldersMutation() {
  return useMutation({
    mutationFn: projectsApi.scanGitBaseFolders,
  });
}
