import type { AgentName, Prompt } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export interface SetPromptTargetInput {
  promptId: string;
  agent: AgentName;
  enabled: boolean;
}

export const promptsApi = {
  list(): Promise<Prompt[]> {
    return invokeCommand<Prompt[]>("list_prompts");
  },

  scan(): Promise<Prompt[]> {
    return invokeCommand<Prompt[]>("scan_prompts");
  },

  setTarget(input: SetPromptTargetInput): Promise<Prompt> {
    return invokeCommand<Prompt>("set_prompt_target", { input });
  },

  openSource(id: string): Promise<void> {
    return invokeCommand<void>("open_prompt_source", { id });
  },

  revealPath(id: string): Promise<void> {
    return invokeCommand<void>("reveal_prompt_path", { id });
  },
};
