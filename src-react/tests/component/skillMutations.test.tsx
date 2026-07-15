import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { skillsApi } from "@/lib/api/skills";
import { projectKeys } from "@/lib/query/projects";
import {
  skillKeys,
  useApplyProjectCustomSkillIntentMutation,
  useSetSkillTargetMutation,
} from "@/lib/query/skills";
import type { Skill } from "@/types";

const cells = {
  "Generic Agent": "source",
  "Claude Code": "none",
  CodeX: "none",
  Copilot: "none",
  OpenCode: "none",
  Pi: "none",
  Qoder: "none",
} as const;

function row(id: string): Skill {
  return {
    kind: "agentCanonical",
    rowKey: id,
    skill: { skillId: id, name: id, desc: "", path: `/${id}`, disabled: false },
    context: { kind: "global" },
    sourceAgent: "Generic Agent",
    cells,
  };
}

function harness() {
  const queryClient = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  const fetchProjects = vi.fn().mockResolvedValue([]);
  queryClient.setQueryDefaults(projectKeys.all, { queryFn: fetchProjects });
  queryClient.setQueryData(projectKeys.all, []);
  new QueryObserver(queryClient, { queryKey: projectKeys.all }).subscribe(() => undefined);
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, fetchProjects, Wrapper };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Skill mutation cache contract", () => {
  it("retained mutations replace the entire authoritative catalog", async () => {
    const { queryClient, Wrapper } = harness();
    queryClient.setQueryData(skillKeys.all, [row("old"), row("stale")]);
    vi.spyOn(skillsApi, "setTarget").mockResolvedValue([row("new")]);
    const { result } = renderHook(() => useSetSkillTargetMutation(), { wrapper: Wrapper });

    await result.current.mutateAsync({
      skillId: "old",
      agent: "Claude Code",
      enabled: true,
    });

    expect(queryClient.getQueryData(skillKeys.all)).toEqual([row("new")]);
  });

  it("failed mutations leave the existing Skill cache untouched", async () => {
    const { queryClient, Wrapper } = harness();
    queryClient.setQueryData(skillKeys.all, [row("existing")]);
    vi.spyOn(skillsApi, "applyProjectCustomIntent").mockRejectedValue(new Error("failed"));
    const { result } = renderHook(() => useApplyProjectCustomSkillIntentMutation(), {
      wrapper: Wrapper,
    });

    await expect(
      result.current.mutateAsync({
        kind: "setTargetEnabled",
        skillId: "custom",
        destination: { kind: "global" },
        enabled: true,
      }),
    ).rejects.toThrow("failed");
    expect(queryClient.getQueryData(skillKeys.all)).toEqual([row("existing")]);
  });

  it("Project custom intent replaces catalog and invalidates Project counts", async () => {
    const { queryClient, fetchProjects, Wrapper } = harness();
    vi.spyOn(skillsApi, "applyProjectCustomIntent").mockResolvedValue({
      changed: true,
      skills: [row("authoritative")],
    });
    const { result } = renderHook(() => useApplyProjectCustomSkillIntentMutation(), {
      wrapper: Wrapper,
    });

    await result.current.mutateAsync({
      kind: "setTargetEnabled",
      skillId: "custom",
      destination: { kind: "global" },
      enabled: false,
    });

    expect(queryClient.getQueryData(skillKeys.all)).toEqual([row("authoritative")]);
    await waitFor(() => expect(fetchProjects).toHaveBeenCalled());
  });
});
