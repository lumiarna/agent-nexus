import { useQuery } from "@tanstack/react-query";

import { sessionsApi } from "@/lib/api/sessions";
import { isTauriRuntime } from "@/lib/runtime";

export const sessionKeys = {
  local: ["sessions", "local"] as const,
  localDetail: (id: string) => ["sessions", "local", id] as const,
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
