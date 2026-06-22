import type { QueryClient } from "@tanstack/react-query";

export const projectKeys = {
  all: ["projects"] as const,
};

const projectSkillKeys = {
  all: ["skills"] as const,
};

const projectSessionKeys = {
  local: ["sessions", "local"] as const,
};

const projectSymlinkInventoryKeys = {
  inventory: ["projects", "projectSymlinkInventory"] as const,
};

export function createDeleteProjectMutationOptions(
  queryClient: Pick<QueryClient, "invalidateQueries">,
  deleteProject: (id: string) => Promise<void>,
) {
  return {
    mutationFn: deleteProject,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: projectKeys.all }),
        queryClient.invalidateQueries({ queryKey: projectSkillKeys.all }),
        queryClient.invalidateQueries({ queryKey: projectSessionKeys.local }),
        queryClient.invalidateQueries({ queryKey: projectSymlinkInventoryKeys.inventory }),
      ]);
    },
  };
}
