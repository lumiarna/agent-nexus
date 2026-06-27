import { invokeCommand } from "./tauri.js";

/** User preference for which Agents are disabled. A disabled Agent is dropped
 *  from the Skill / Prompt Agent Matrix and the assets it sources are hidden;
 *  its Agent Capability Surface still exists. Values are canonical Agent names. */
export interface AgentDisplayPreferences {
  disabled: string[];
}

export const agentPreferencesApi = {
  get(): Promise<AgentDisplayPreferences> {
    return invokeCommand<AgentDisplayPreferences>("get_disabled_agents");
  },
  set(preferences: AgentDisplayPreferences): Promise<AgentDisplayPreferences> {
    return invokeCommand<AgentDisplayPreferences>("set_disabled_agents", { preferences });
  },
};
