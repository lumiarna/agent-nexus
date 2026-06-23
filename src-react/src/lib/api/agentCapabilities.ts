import { invokeCommand } from "@/lib/api/tauri";

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
  name: string;
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
};
