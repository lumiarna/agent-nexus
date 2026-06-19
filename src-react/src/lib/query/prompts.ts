import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { promptsApi, type SetPromptTargetInput } from "@/lib/api/prompts";
import { isTauriRuntime } from "@/lib/runtime";
import type { Prompt } from "@/types";

export const promptKeys = {
  all: ["prompts"] as const,
};

function replacePrompt(current: Prompt[] | undefined, next: Prompt): Prompt[] {
  if (!current) return [next];
  return current.map((prompt) => (prompt.id === next.id ? next : prompt));
}

export function usePromptsQuery() {
  return useQuery({
    queryKey: promptKeys.all,
    queryFn: promptsApi.scan,
    enabled: isTauriRuntime(),
    staleTime: 30 * 1000,
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
