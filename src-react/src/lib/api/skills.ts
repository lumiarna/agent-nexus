import type { AgentName, Skill } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export interface SetSkillTargetInput {
  skillId: string;
  agent: AgentName;
  enabled: boolean;
}

export interface MoveSkillSourceInput {
  skillId: string;
  agent: AgentName;
}

export interface SetProjectSkillProjectInput {
  skillId: string;
  targetProjectId: string;
  defaultAgent: AgentName;
  enabled: boolean;
}

export interface SetProjectSkillTargetInput {
  skillId: string;
  targetProjectId: string;
  agent: AgentName;
  enabled: boolean;
}

export const skillsApi = {
  list(): Promise<Skill[]> {
    return invokeCommand<Skill[]>("list_skills");
  },

  scan(): Promise<Skill[]> {
    return invokeCommand<Skill[]>("scan_skills");
  },

  setTarget(input: SetSkillTargetInput): Promise<Skill> {
    return invokeCommand<Skill>("set_skill_target", { input });
  },

  moveSource(input: MoveSkillSourceInput): Promise<Skill> {
    return invokeCommand<Skill>("move_skill_source", { input });
  },

  setDisabled(id: string, disabled: boolean): Promise<Skill> {
    return invokeCommand<Skill>("set_skill_disabled", { id, disabled });
  },

  openSource(id: string): Promise<void> {
    return invokeCommand<void>("open_skill_source", { id });
  },

  revealPath(id: string): Promise<void> {
    return invokeCommand<void>("reveal_skill_path", { id });
  },

  /** Source-side: propagate a Project custom Skill to (or cancel it from) a
   *  target Project. Returns the full skill list so projection rows refresh. */
  setProjectSkillProject(input: SetProjectSkillProjectInput): Promise<Skill[]> {
    return invokeCommand<Skill[]>("set_project_skill_project", { input });
  },

  /** Target-side: toggle one Agent placement inside an incoming target
   *  Project Skill row. Returns the full skill list. */
  setProjectSkillTarget(input: SetProjectSkillTargetInput): Promise<Skill[]> {
    return invokeCommand<Skill[]>("set_project_skill_target", { input });
  },
};
