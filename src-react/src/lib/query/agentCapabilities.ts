import { useQuery } from "@tanstack/react-query";

import { agentCapabilitiesApi } from "@/lib/api/agentCapabilities";
import { isTauriRuntime } from "@/lib/runtime";

export const agentCapabilityKeys = {
  all: ["agentCapabilities"] as const,
};

export function useAgentCapabilitiesQuery() {
  return useQuery({
    queryKey: agentCapabilityKeys.all,
    queryFn: () => agentCapabilitiesApi.list(),
    enabled: isTauriRuntime(),
  });
}
