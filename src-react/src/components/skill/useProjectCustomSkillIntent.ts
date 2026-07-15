import { toast } from "sonner";

import { useApplyProjectCustomSkillIntentMutation } from "@/lib/query/skills";
import type { ProjectCustomSkillIntent } from "@/types";

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return "Unexpected error";
}

/** Narrow caller-side adapter shared by both Skill surfaces. It delegates one
 * typed user intent to core; no page learns default-Agent, projection joins,
 * multi-placement loops, or compensation rules. */
export function useProjectCustomSkillIntent(desktop: boolean) {
  const mutation = useApplyProjectCustomSkillIntentMutation();
  return async (intent: ProjectCustomSkillIntent) => {
    if (!desktop) {
      toast("Desktop runtime required for changing skill placements");
      return;
    }
    try {
      await mutation.mutateAsync(intent);
      if (intent.kind === "setTargetEnabled") {
        toast(intent.enabled ? "Skill propagated" : "Skill placement removed");
      } else {
        toast(intent.enabled ? "Target linked" : "Target removed");
      }
    } catch (error) {
      toast(errorMessage(error));
    }
  };
}
