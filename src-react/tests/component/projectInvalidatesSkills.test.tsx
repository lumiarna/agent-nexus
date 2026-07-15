import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { projectsApi } from "@/lib/api/projects";
import {
  useRecordProjectMutation,
  useRecordProjectsMutation,
  useReorderProjectsMutation,
} from "@/lib/query/projects";
import { skillKeys } from "@/lib/query/skills";
import type { Project } from "@/types";

const project: Project = {
  id: "p1",
  name: "P1",
  status: "active",
  path: "/p1",
  sessionsDir: ".sessions",
  skills: 0,
  prompts: 0,
  sessions: 0,
  sync: 0,
  key: "p1",
};

function harness() {
  const queryClient = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  const fetchSkills = vi.fn().mockResolvedValue([]);
  queryClient.setQueryDefaults(skillKeys.all, { queryFn: fetchSkills });
  queryClient.setQueryData(skillKeys.all, []);
  new QueryObserver(queryClient, { queryKey: skillKeys.all }).subscribe(() => undefined);
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { fetchSkills, Wrapper };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Project writes invalidate eager Skill destinations", () => {
  it("record and batch record invalidate the Skill catalog", async () => {
    vi.spyOn(projectsApi, "record").mockResolvedValue(project);
    const single = harness();
    const { result: singleHook } = renderHook(() => useRecordProjectMutation(), {
      wrapper: single.Wrapper,
    });
    await singleHook.current.mutateAsync("/p1");
    await waitFor(() => expect(single.fetchSkills).toHaveBeenCalled());

    const batch = harness();
    const { result: batchHook } = renderHook(() => useRecordProjectsMutation(), {
      wrapper: batch.Wrapper,
    });
    await batchHook.current.mutateAsync(["/p1"]);
    await waitFor(() => expect(batch.fetchSkills).toHaveBeenCalled());
  });

  it("reorder invalidates the Skill catalog because destination order is eager", async () => {
    vi.spyOn(projectsApi, "reorder").mockResolvedValue([project]);
    const { fetchSkills, Wrapper } = harness();
    const { result } = renderHook(() => useReorderProjectsMutation(), { wrapper: Wrapper });
    await result.current.mutateAsync(["p1"]);
    await waitFor(() => expect(fetchSkills).toHaveBeenCalled());
  });
});
