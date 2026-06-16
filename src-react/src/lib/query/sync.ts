import { useQuery } from "@tanstack/react-query";

import { syncApi } from "@/lib/api/sync";

export const syncKeys = {
  projectSymlinks: ["sync", "projectSymlinks"] as const,
};

export function useProjectSymlinksQuery() {
  return useQuery({
    queryKey: syncKeys.projectSymlinks,
    queryFn: syncApi.listProjectSymlinks,
  });
}
