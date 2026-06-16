import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { skillsApi, type SetSkillTargetInput } from "@/lib/api/skills";
import { projectKeys } from "@/lib/query/projects";
import { isTauriRuntime } from "@/lib/runtime";
import type { Skill } from "@/types";

export const skillKeys = {
  all: ["skills"] as const,
};

function replaceSkill(current: Skill[] | undefined, next: Skill): Skill[] {
  if (!current) return [next];
  return current.map((skill) => (skill.id === next.id ? next : skill));
}

export function useSkillsQuery() {
  return useQuery({
    queryKey: skillKeys.all,
    queryFn: skillsApi.list,
    enabled: isTauriRuntime(),
  });
}

export function useScanSkillsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: skillsApi.scan,
    onSuccess: (skills) => {
      queryClient.setQueryData(skillKeys.all, skills);
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
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
