import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { projectSymlinksApi } from "../api/projectSymlinks.js";
import {
  createDeleteProjectSymlinkMutationOptions,
  projectSymlinkKeys,
} from "./projectSymlinkInventory.js";

export { invalidateProjectSymlinkInventory, projectSymlinkKeys } from "./projectSymlinkInventory.js";

export function useProjectSymlinksQuery() {
  return useQuery({
    queryKey: projectSymlinkKeys.inventory,
    queryFn: projectSymlinksApi.list,
  });
}

export function useDeleteProjectSymlinkMutation() {
  const queryClient = useQueryClient();
  return useMutation(
    createDeleteProjectSymlinkMutationOptions(queryClient, (targetPath) =>
      projectSymlinksApi.delete(targetPath),
    ),
  );
}
