import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { sessionsApi } from "@/lib/api/sessions";
import { skillsApi } from "@/lib/api/skills";
import { promptsApi } from "@/lib/api/prompts";
import { projectKeys } from "@/lib/query/projects";
import { useLocalSessionsQuery, useCloudSessionsQuery } from "@/lib/query/sessions";
import { useSkillsQuery } from "@/lib/query/skills";
import { usePromptsQuery } from "@/lib/query/prompts";

beforeEach(() => {
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
});

afterEach(() => {
  cleanup();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  vi.restoreAllMocks();
});

interface Harness {
  queryClient: QueryClient;
  fetchProjects: ReturnType<typeof vi.fn>;
  Wrapper: ({ children }: { children: ReactNode }) => React.JSX.Element;
}

function makeHarness(): Harness {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  const fetchProjects = vi.fn().mockResolvedValue([]);
  queryClient.setQueryDefaults(projectKeys.all, { queryFn: fetchProjects });
  queryClient.setQueryData(projectKeys.all, []);
  new QueryObserver(queryClient, { queryKey: projectKeys.all }).subscribe(() => undefined);
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, fetchProjects, Wrapper };
}

describe("scan queries invalidate the projects list", () => {
  it("useLocalSessionsQuery triggers a projects refetch", async () => {
    vi.spyOn(sessionsApi, "scanLocal").mockResolvedValue([]);
    const { fetchProjects, Wrapper } = makeHarness();

    renderHook(() => useLocalSessionsQuery(), { wrapper: Wrapper });

    await waitFor(() => expect(fetchProjects).toHaveBeenCalled());
  });

  it("useCloudSessionsQuery triggers a projects refetch", async () => {
    vi.spyOn(sessionsApi, "scanCloud").mockResolvedValue([]);
    const { fetchProjects, Wrapper } = makeHarness();

    renderHook(() => useCloudSessionsQuery(), { wrapper: Wrapper });

    await waitFor(() => expect(fetchProjects).toHaveBeenCalled());
  });

  it("useSkillsQuery reads without triggering a projects refetch (scan does)", async () => {
    vi.spyOn(skillsApi, "list").mockResolvedValue([]);
    const { fetchProjects, Wrapper } = makeHarness();

    renderHook(() => useSkillsQuery(), { wrapper: Wrapper });

    // list_skills is a read — it does not invalidate projects
    await waitFor(() => expect(fetchProjects).not.toHaveBeenCalled());
  });

  it("usePromptsQuery triggers a projects refetch", async () => {
    vi.spyOn(promptsApi, "scan").mockResolvedValue([]);
    const { fetchProjects, Wrapper } = makeHarness();

    renderHook(() => usePromptsQuery(), { wrapper: Wrapper });

    await waitFor(() => expect(fetchProjects).toHaveBeenCalled());
  });
});