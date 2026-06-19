import type { QueryClient } from "@tanstack/react-query";

export const projectSymlinkKeys = {
  inventory: ["projects", "projectSymlinkInventory"] as const,
};

export function invalidateProjectSymlinkInventory(
  queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: projectSymlinkKeys.inventory });
}

export function createDeleteProjectSymlinkMutationOptions(
  queryClient: Pick<QueryClient, "invalidateQueries">,
  deleteProjectSymlink: (targetPath: string) => Promise<void>,
) {
  return {
    mutationFn: deleteProjectSymlink,
    onSuccess: () => invalidateProjectSymlinkInventory(queryClient),
  };
}
