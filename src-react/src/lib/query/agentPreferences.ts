import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { agentPreferencesApi } from "@/lib/api/agentPreferences";
import type { AgentDisplayPreferences } from "@/lib/api/agentPreferences";
import { isTauriRuntime } from "@/lib/runtime";
import { AGENT_ORDER } from "@/lib/tokens";
import type { AgentName } from "@/types";

export const agentPreferenceKeys = {
  disabled: ["agentPreferences", "disabled"] as const,
};

/** Canonical-leftmost Agent used when no Default Global entry Agent is set. */
export const DEFAULT_GLOBAL_ENTRY_FALLBACK: AgentName = "Generic Agent";

export function useAgentPreferencesQuery() {
  return useQuery({
    queryKey: agentPreferenceKeys.disabled,
    queryFn: agentPreferencesApi.get,
    enabled: isTauriRuntime(),
  });
}

export function useSetAgentPreferencesMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (preferences: AgentDisplayPreferences) => agentPreferencesApi.set(preferences),
    onSuccess: (preferences) => {
      queryClient.setQueryData(agentPreferenceKeys.disabled, preferences);
    },
  });
}

/** The full Agent display preferences. Empty in the browser preview (no backend). */
export function useAgentPreferences(): AgentDisplayPreferences {
  const { data } = useAgentPreferencesQuery();
  return data ?? { disabled: [] };
}

/** The set of disabled Agents. Empty in the browser preview (no backend). */
export function useDisabledAgents(): Set<AgentName> {
  const { data } = useAgentPreferencesQuery();
  return useMemo(() => new Set((data?.disabled ?? []) as AgentName[]), [data]);
}

/** Enabled Agents in canonical order — the set rendered in the Agent Matrix. */
export function useEnabledAgents(): AgentName[] {
  const disabled = useDisabledAgents();
  return useMemo(() => AGENT_ORDER.filter((agent) => !disabled.has(agent)), [disabled]);
}

/** Default Global entry Agent for propagating Project custom Skills, falling
 *  back to the canonical-leftmost Agent when unset. */
export function useDefaultGlobalEntryAgent(): AgentName {
  const { data } = useAgentPreferencesQuery();
  return (data?.defaultGlobalEntryAgent as AgentName) ?? DEFAULT_GLOBAL_ENTRY_FALLBACK;
}
