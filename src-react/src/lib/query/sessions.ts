import { useQuery } from "@tanstack/react-query";

import { sessionsApi } from "@/lib/api/sessions";
import { isTauriRuntime } from "@/lib/runtime";

export const sessionKeys = {
  local: ["sessions", "local"] as const,
  localDetail: (id: string) => ["sessions", "local", id] as const,
  cloud: ["sessions", "cloud"] as const,
  cloudDetail: (id: string) => ["sessions", "cloud", id] as const,
};

export function useLocalSessionsQuery() {
  return useQuery({
    queryKey: sessionKeys.local,
    queryFn: sessionsApi.scanLocal,
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
  return useQuery({
    queryKey: sessionKeys.cloud,
    queryFn: sessionsApi.scanCloud,
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
