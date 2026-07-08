import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  skillsApi,
  type SetProjectSkillProjectInput,
  type SetProjectSkillTargetInput,
  type SetSkillTargetInput,
} from "@/lib/api/skills";
import { isTauriRuntime } from "@/lib/runtime";
import { projectKeys } from "@/lib/query/projects";
import type { Skill } from "@/types";

export const skillKeys = {
  all: ["skills"] as const,
};

function replaceSkill(current: Skill[] | undefined, next: Skill): Skill[] {
  if (!current) return [next];
  return current.map((skill) => (skill.id === next.id ? next : skill));
}

export function useSkillsQuery() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: skillKeys.all,
    queryFn: async () => {
      const skills = await skillsApi.scan();
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
      return skills;
    },
    enabled: isTauriRuntime(),
  });
}

export function useSetSkillTargetMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetSkillTargetInput) => skillsApi.setTarget(input),
    onSuccess: (skill) => {
      queryClient.setQueryData<Skill[]>(skillKeys.all, (current) => replaceSkill(current, skill));
    },
  });
}

export function useSetSkillDisabledMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, disabled }: { id: string; disabled: boolean }) =>
      skillsApi.setDisabled(id, disabled),
    onSuccess: (skill) => {
      queryClient.setQueryData<Skill[]>(skillKeys.all, (current) => replaceSkill(current, skill));
    },
  });
}

/** Cross-Project propagation mutations return the full skill list (projection
 *  rows are derived server-side), so they replace the whole cache and also
 *  invalidate the Project list — incoming rows change Project skill counts. */
function setFullSkillList(_current: Skill[] | undefined, next: Skill[]): Skill[] {
  return next;
}

export function useSetProjectSkillProjectMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetProjectSkillProjectInput) => skillsApi.setProjectSkillProject(input),
    onSuccess: (skills) => {
      queryClient.setQueryData<Skill[]>(skillKeys.all, (current) =>
        setFullSkillList(current, skills),
      );
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useSetProjectSkillTargetMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetProjectSkillTargetInput) => skillsApi.setProjectSkillTarget(input),
    onSuccess: (skills) => {
      queryClient.setQueryData<Skill[]>(skillKeys.all, (current) =>
        setFullSkillList(current, skills),
      );
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}
