import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { projectsApi } from "@/lib/api/projects";

export const projectKeys = {
  all: ["projects"] as const,
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
