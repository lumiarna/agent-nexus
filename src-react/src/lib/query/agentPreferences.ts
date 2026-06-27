import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { agentPreferencesApi } from "@/lib/api/agentPreferences";
import { isTauriRuntime } from "@/lib/runtime";
import { AGENT_ORDER } from "@/lib/tokens";
import type { AgentName } from "@/types";

export const agentPreferenceKeys = {
  disabled: ["agentPreferences", "disabled"] as const,
};

export function useDisabledAgentsQuery() {
  return useQuery({
    queryKey: agentPreferenceKeys.disabled,
    queryFn: agentPreferencesApi.get,
    enabled: isTauriRuntime(),
  });
}

export function useSetDisabledAgentsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: agentPreferencesApi.set,
    onSuccess: (preferences) => {
      queryClient.setQueryData(agentPreferenceKeys.disabled, preferences);
    },
  });
}

/** The set of disabled Agents. Empty in the browser preview (no backend). */
export function useDisabledAgents(): Set<AgentName> {
  const { data } = useDisabledAgentsQuery();
  return useMemo(() => new Set((data?.disabled ?? []) as AgentName[]), [data]);
}

/** Enabled Agents in canonical order — the set rendered in the Agent Matrix. */
export function useEnabledAgents(): AgentName[] {
  const disabled = useDisabledAgents();
  return useMemo(() => AGENT_ORDER.filter((agent) => !disabled.has(agent)), [disabled]);
}
