import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  promptsApi,
  type MovePromptSourceInput,
  type SetPromptTargetInput,
} from "@/lib/api/prompts";
import { isTauriRuntime } from "@/lib/runtime";
import { projectKeys } from "@/lib/query/projects";
import type { Prompt } from "@/types";

export const promptKeys = {
  all: ["prompts"] as const,
};

function replacePrompt(current: Prompt[] | undefined, next: Prompt): Prompt[] {
  if (!current) return [next];
  return current.map((prompt) => (prompt.id === next.id ? next : prompt));
}

export function usePromptsQuery() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: promptKeys.all,
    queryFn: async () => {
      const prompts = await promptsApi.scan();
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
      return prompts;
    },
    enabled: isTauriRuntime(),
  });
}

export function useSetPromptTargetMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetPromptTargetInput) => promptsApi.setTarget(input),
    onSuccess: (prompt) => {
      queryClient.setQueryData<Prompt[]>(promptKeys.all, (current) =>
        replacePrompt(current, prompt),
      );
    },
  });
}

export function useMovePromptSourceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: MovePromptSourceInput) => promptsApi.moveSource(input),
    onSuccess: (prompt) => {
      queryClient.setQueryData<Prompt[]>(promptKeys.all, (current) =>
        replacePrompt(current, prompt),
      );
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}
