import { useQuery, useQueryClient } from "@tanstack/react-query";

import { sessionsApi } from "@/lib/api/sessions";
import { isTauriRuntime } from "@/lib/runtime";
import { projectKeys } from "@/lib/query/projects";

export const sessionKeys = {
  local: ["sessions", "local"] as const,
  localDetail: (id: string) => ["sessions", "local", id] as const,
  cloud: ["sessions", "cloud"] as const,
  cloudDetail: (id: string) => ["sessions", "cloud", id] as const,
};

export function useLocalSessionsQuery() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: sessionKeys.local,
    queryFn: async () => {
      const sessions = await sessionsApi.scanLocal();
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
      return sessions;
    },
    enabled: isTauriRuntime(),
  });
}

export function useLocalSessionQuery(id: string | null, enabled: boolean) {
  return useQuery({
    queryKey: id ? sessionKeys.localDetail(id) : ["sessions", "local", "detail", "none"],
    queryFn: () => sessionsApi.getLocal(id ?? ""),
    enabled: isTauriRuntime() && enabled && id != null,
  });
}

export function useCloudSessionsQuery() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: sessionKeys.cloud,
    queryFn: async () => {
      const sessions = await sessionsApi.scanCloud();
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
      return sessions;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCloudSessionQuery(id: string | null, enabled: boolean) {
  return useQuery({
    queryKey: id ? sessionKeys.cloudDetail(id) : ["sessions", "cloud", "detail", "none"],
    queryFn: () => sessionsApi.getCloud(id ?? ""),
    enabled: isTauriRuntime() && enabled && id != null,
  });
}
