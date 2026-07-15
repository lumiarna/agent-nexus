import type {
  AgentName,
  ProjectCustomSkillIntent,
  ProjectCustomSkillMutationResult,
  Skill,
} from "@/types";
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

export const skillsApi = {
  list(): Promise<Skill[]> {
    return invokeCommand<Skill[]>("list_skills");
  },

  scan(): Promise<Skill[]> {
    return invokeCommand<Skill[]>("scan_skills");
  },

  setTarget(input: SetSkillTargetInput): Promise<Skill[]> {
    return invokeCommand<Skill[]>("set_skill_target", { input });
  },

  moveSource(input: MoveSkillSourceInput): Promise<Skill[]> {
    return invokeCommand<Skill[]>("move_skill_source", { input });
  },

  setDisabled(id: string, disabled: boolean): Promise<Skill[]> {
    return invokeCommand<Skill[]>("set_skill_disabled", { id, disabled });
  },

  openSource(id: string): Promise<void> {
    return invokeCommand<void>("open_skill_source", { id });
  },

  revealPath(id: string): Promise<void> {
    return invokeCommand<void>("reveal_skill_path", { id });
  },

  applyProjectCustomIntent(
    intent: ProjectCustomSkillIntent,
  ): Promise<ProjectCustomSkillMutationResult> {
    return invokeCommand<ProjectCustomSkillMutationResult>(
      "apply_project_custom_skill_intent",
      { intent },
    );
  },
};
