import type { AgentName } from "../../types/index.js";
import { invokeCommand } from "./tauri.js";

export interface SkillCapabilitySurface {
  globalDir: string;
  projectDir: string;
}

export interface PromptCapabilitySurface {
  globalFile: string;
  projectFile?: string | null;
}

export interface ProviderCapabilitySurface {
  providerId: string;
  credentialHint?: string | null;
}

export interface AgentCapabilitySurface {
  name: AgentName;
  abbr: string;
  color: string;
  configDir: string;
  skill?: SkillCapabilitySurface | null;
  prompt?: PromptCapabilitySurface | null;
  provider?: ProviderCapabilitySurface | null;
}

export const agentCapabilitiesApi = {
  list(): Promise<AgentCapabilitySurface[]> {
    return invokeCommand<AgentCapabilitySurface[]>("list_agent_capabilities");
  },

  openConfigRoot(name: AgentName): Promise<void> {
    return invokeCommand<void>("open_agent_config_root", { name });
  },
};
