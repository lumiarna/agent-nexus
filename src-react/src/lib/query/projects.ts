import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { projectsApi } from "@/lib/api/projects";
import { createDeleteProjectMutationOptions } from "@/lib/query/projectDeletion";
import { projectSymlinkKeys } from "@/lib/query/projectSymlinkInventory";
import { promptKeys } from "@/lib/query/prompts";
import { sessionKeys } from "@/lib/query/sessions";
import { skillKeys } from "@/lib/query/skills";
import type { Project, ProjectDefaults } from "@/types";

export { createDeleteProjectMutationOptions };

export const projectKeys = {
  all: ["projects"] as const,
  gitBaseFolders: ["projects", "gitBaseFolders"] as const,
  defaults: ["projects", "defaults"] as const,
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

export function useDeleteProjectMutation() {
  const queryClient = useQueryClient();

  return useMutation(
    createDeleteProjectMutationOptions(queryClient, projectsApi.delete),
  );
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

export function useSetProjectCustomSkillsDirsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ projectId, dirs }: { projectId: string; dirs: string[] }) =>
      projectsApi.setCustomSkillsDirs(projectId, dirs),
    onSuccess: async (project: Project) => {
      queryClient.setQueryData<Project[]>(projectKeys.all, (current) =>
        current ? current.map((p) => (p.id === project.id ? project : p)) : current,
      );
      // Custom dirs change the Project custom Skill set — rescan on next read.
      await queryClient.invalidateQueries({ queryKey: skillKeys.all });
    },
  });
}

export function useSetProjectExtraPromptFilesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ projectId, files }: { projectId: string; files: string[] }) =>
      projectsApi.setExtraPromptFiles(projectId, files),
    onSuccess: async (project: Project) => {
      queryClient.setQueryData<Project[]>(projectKeys.all, (current) =>
        current ? current.map((p) => (p.id === project.id ? project : p)) : current,
      );
      // Extra prompt files widen the Project prompt scan — rescan on next read.
      await queryClient.invalidateQueries({ queryKey: promptKeys.all });
    },
  });
}

export function useSetProjectSessionsDirMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ projectId, dir }: { projectId: string; dir: string }) =>
      projectsApi.setSessionsDir(projectId, dir),
    onSuccess: async (project: Project) => {
      queryClient.setQueryData<Project[]>(projectKeys.all, (current) =>
        current ? current.map((p) => (p.id === project.id ? project : p)) : current,
      );
      // The Session Directory moved — local sessions resolve from the new path.
      await queryClient.invalidateQueries({ queryKey: sessionKeys.local });
    },
  });
}

export function useProjectDefaultsQuery() {
  return useQuery({
    queryKey: projectKeys.defaults,
    queryFn: projectsApi.getDefaults,
  });
}

/** Shared cache update for the three Project Defaults setters — each returns the
 *  full updated defaults, so we just write it back to the single defaults query. */
function useSetProjectDefaultsMutation<TVars>(
  mutationFn: (vars: TVars) => Promise<ProjectDefaults>,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn,
    onSuccess: (defaults: ProjectDefaults) => {
      queryClient.setQueryData<ProjectDefaults>(projectKeys.defaults, defaults);
    },
  });
}

export function useSetDefaultCustomSkillsDirsMutation() {
  return useSetProjectDefaultsMutation((dirs: string[]) =>
    projectsApi.setDefaultCustomSkillsDirs(dirs),
  );
}

export function useSetDefaultExtraPromptFilesMutation() {
  return useSetProjectDefaultsMutation((files: string[]) =>
    projectsApi.setDefaultExtraPromptFiles(files),
  );
}

export function useSetDefaultSessionsDirMutation() {
  return useSetProjectDefaultsMutation((dir: string) =>
    projectsApi.setDefaultSessionsDir(dir),
  );
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
