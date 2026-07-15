import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  skillsApi,
  type MoveSkillSourceInput,
  type SetSkillTargetInput,
} from "@/lib/api/skills";
import { isTauriRuntime } from "@/lib/runtime";
import { projectKeys } from "@/lib/query/projects";
import type { ProjectCustomSkillIntent, Skill } from "@/types";

export const skillKeys = {
  all: ["skills"] as const,
};

function replaceCatalog(queryClient: ReturnType<typeof useQueryClient>, skills: Skill[]) {
  queryClient.setQueryData<Skill[]>(skillKeys.all, skills);
}

export function useSkillsQuery() {
  return useQuery({
    queryKey: skillKeys.all,
    queryFn: () => skillsApi.list(),
    enabled: isTauriRuntime(),
  });
}

/** Scan (re-discover) skills: writes new/changed sources into the database
 *  and returns the authoritative catalog. Use via the Refresh button, not
 *  as a normal query — it acquires the mutation lock and modifies state. */
export function useScanSkillsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => skillsApi.scan(),
    onSuccess: (skills) => {
      replaceCatalog(queryClient, skills);
      // Scanning discovers new Project custom Skills, which affects
      // incoming row counts visible on Project cards.
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useSetSkillTargetMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: SetSkillTargetInput) => skillsApi.setTarget(input),
    onSuccess: (skills) => replaceCatalog(queryClient, skills),
  });
}

export function useMoveSkillSourceMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: MoveSkillSourceInput) => skillsApi.moveSource(input),
    onSuccess: (skills) => replaceCatalog(queryClient, skills),
  });
}

export function useSetSkillDisabledMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, disabled }: { id: string; disabled: boolean }) =>
      skillsApi.setDisabled(id, disabled),
    onSuccess: (skills) => replaceCatalog(queryClient, skills),
  });
}

/** The sole Project custom Skill write adapter. Core owns identity, default
 * Agent selection, fan-out, withdrawal, compensation, and the resulting eager
 * read model; the hook only installs the authoritative catalog. */
export function useApplyProjectCustomSkillIntentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (intent: ProjectCustomSkillIntent) =>
      skillsApi.applyProjectCustomIntent(intent),
    onSuccess: (result) => {
      replaceCatalog(queryClient, result.skills);
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}
