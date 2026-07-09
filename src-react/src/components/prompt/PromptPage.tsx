import { useState } from "react";
import type { MouseEvent } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import {
  AgentMatrixCells,
  MatrixLegend,
  SourceBadge,
  isMoveSourceModifier,
} from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Card, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { ScreenScroll } from "@/components/shell/screen";
import { promptsApi } from "@/lib/api/prompts";
import { AGENTS } from "@/config/agents";
import { useProjectsQuery } from "@/lib/query/projects";
import {
  useMovePromptSourceMutation,
  usePromptsQuery,
  useSetPromptTargetMutation,
} from "@/lib/query/prompts";
import { useDisabledAgents, useEnabledAgents } from "@/lib/query/agentPreferences";
import { isTauriRuntime } from "@/lib/runtime";
import { srcAgentOf } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { AgentName, Prompt } from "@/types";

type Scope = "global" | "project";

/** Project prompts collapse to the two repo-root files — AGENTS.md (the
 *  cross-tool standard) and CLAUDE.md — so the matrix only spans these agents. */
const PROJECT_PROMPT_AGENTS: AgentName[] = AGENTS.filter(
  (agent) => agent.projectPromptFile,
).map((agent) => agent.name);

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Unexpected error";
}

export function PromptPage() {
  const desktop = isTauriRuntime();
  const promptsQuery = usePromptsQuery();
  const projectsQuery = useProjectsQuery();
  const setPromptTarget = useSetPromptTargetMutation();
  const movePromptSource = useMovePromptSourceMutation();
  const enabledAgents = useEnabledAgents();
  const disabledAgents = useDisabledAgents();
  const [scope, setScope] = useState<Scope>("global");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const prompts = promptsQuery.data ?? [];
  const projects = (projectsQuery.data ?? []).filter((p) => p.status === "active");
  const pageError =
    desktop && promptsQuery.error ? getErrorMessage(promptsQuery.error) : null;
  const isLoading = desktop && promptsQuery.isLoading;
  const isRefreshing = desktop && promptsQuery.isFetching;

  async function scan() {
    if (!desktop) {
      toast("Desktop runtime required for scanning");
      return;
    }

    try {
      const result = await promptsQuery.refetch();
      if (result.data) {
        toast(
          `Refreshed ${result.data.length} ${result.data.length === 1 ? "prompt" : "prompts"}`,
        );
      }
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleCell(
    prompt: Prompt,
    agent: AgentName,
    event: MouseEvent<HTMLSpanElement>,
  ) {
    if (prompt.cells[agent] === "source") return;

    if (!desktop) {
      toast("Desktop runtime required for changing prompt targets");
      return;
    }

    try {
      if (isMoveSourceModifier(event)) {
        await movePromptSource.mutateAsync({ promptId: prompt.id, agent });
        toast(`Source moved to ${agent}`);
        return;
      }

      await setPromptTarget.mutateAsync({
        promptId: prompt.id,
        agent,
        enabled: prompt.cells[agent] !== "target",
      });
      toast(prompt.cells[agent] === "target" ? "Target removed" : "Target linked");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openSource(prompt: Prompt) {
    if (!desktop) {
      toast(`Open source · ${prompt.path}`);
      return;
    }

    try {
      await promptsApi.openSource(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealPath(prompt: Prompt) {
    if (!desktop) {
      toast(`Reveal in file manager · ${prompt.path}`);
      return;
    }

    try {
      await promptsApi.revealPath(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const isProj = scope === "project";
  const q = search.trim().toLowerCase();
  const projectCounts = new Map<string, number>();
  for (const prompt of prompts) {
    if (prompt.scope !== "project" || !prompt.projectId) continue;
    projectCounts.set(prompt.projectId, (projectCounts.get(prompt.projectId) ?? 0) + 1);
  }
  let set = prompts.filter((p) =>
    isProj
      ? p.scope === "project" && (projectId === null || p.projectId === projectId)
      : p.scope !== "project",
  );
  if (q) set = set.filter((p) => p.content.toLowerCase().includes(q));
  // Hide Prompts sourced by a disabled Agent.
  set = set.filter((p) => !disabledAgents.has(srcAgentOf(p.cells)));
  const matrixAgents = isProj
    ? PROJECT_PROMPT_AGENTS.filter((agent) => !disabledAgents.has(agent))
    : enabledAgents;

  let emptyTitle = "";
  let emptyBody = "";
  if (q && set.length === 0) {
    emptyTitle = "No matching prompts";
    emptyBody = `No prompt content matches “${search}” in this scope.`;
  } else if (isProj) {
    const pn = projectId === null ? "any project" : projects.find((p) => p.id === projectId)?.name;
    emptyTitle = "No project prompts";
    emptyBody = `No project-scoped prompts recorded for ${pn}.`;
  } else {
    emptyTitle = "No global prompts";
    emptyBody = "No global prompt files discovered across prompt-capable agents yet.";
  }

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
            Prompt
          </h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Shared prompt assets · Agent Matrix drives distribution
          </p>
        </div>
        <div className="flex items-center gap-2.5">
          <Button
            variant="subtle"
            size="sm"
            onClick={() => void scan()}
            disabled={isRefreshing}
          >
            <RefreshCw size={14} className={cn(isRefreshing && "animate-spin")} />
            {isRefreshing ? "Refreshing..." : "Refresh"}
          </Button>
          <Segmented<Scope>
            options={[
              { value: "global", label: "Global" },
              { value: "project", label: "Project" },
            ]}
            value={scope}
            onChange={setScope}
          />
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-center gap-3.5">
        <Input
          className="min-w-[240px] flex-1 rounded-full bg-nexus-card px-[13px] text-[13px]"
          placeholder="Search by content"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <MatrixLegend />
      </div>

      {isProj ? (
        <div className="mt-3.5 flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-[#b3a999]">Project</span>
          <Chip active={projectId === null} onClick={() => setProjectId(null)}>
            All
          </Chip>
          {projects.map((p) => (
            <Chip
              key={p.id}
              active={projectId === p.id}
              onClick={() => setProjectId(p.id)}
            >
              <span>{p.name}</span>
              {projectCounts.get(p.id) ? (
                <span className="ml-1 opacity-80">{projectCounts.get(p.id)}</span>
              ) : null}
            </Chip>
          ))}
        </div>
      ) : null}

      <Card className="mt-4 overflow-hidden">
        <div
          className="grid items-center gap-4 border-b border-nexus-panel bg-nexus-sand px-5 py-3 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
          style={{ gridTemplateColumns: "1fr 196px 132px" }}
        >
          <div>Prompt</div>
          <div className="text-center">Distribution</div>
          <div className="text-right">Source file</div>
        </div>
        {isLoading && set.length === 0 ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">Scanning prompts</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">
              Reading prompt files from agent capability surfaces.
            </div>
          </div>
        ) : pageError ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-nexus-crit">Prompt scan failed</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{pageError}</div>
          </div>
        ) : set.length > 0 ? (
          set.map((p) => (
            <div
              key={p.id}
              className="grid items-center gap-4 border-b border-[#f3eee5] px-5 py-[14px]"
              style={{ gridTemplateColumns: "1fr 196px 132px" }}
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-[14px] font-bold text-nexus-ink">{p.name}</span>
                  <SourceBadge agent={srcAgentOf(p.cells)} />
                </div>
                <div className="mt-[3px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#a99a89]">
                  {p.path}
                </div>
              </div>
              <AgentMatrixCells
                cells={p.cells}
                agents={matrixAgents}
                onToggle={(a, event) => void toggleCell(p, a, event)}
              />
              <div className="flex flex-col items-end gap-[5px]">
                <span
                  onClick={() => void openSource(p)}
                  className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-nexus-accent hover:underline"
                >
                  Open source
                </span>
                <span
                  onClick={() => void revealPath(p)}
                  className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-[#a99a89] hover:underline"
                >
                  Reveal path
                </span>
              </div>
            </div>
          ))
        ) : (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">{emptyTitle}</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{emptyBody}</div>
          </div>
        )}
      </Card>

      <p className="mt-3.5 text-[11.5px] text-[#b3a999]">
        {isProj ? (
          <>
            Project prompts live at the repo root and collapse to two files —{" "}
            <span className="font-mono">AGENTS.md</span> (
            <b className="text-[#9a8f80]">Generic Agent</b>) and{" "}
            <span className="font-mono">CLAUDE.md</span> (
            <b className="text-[#9a8f80]">Claude Code</b>). Each row has exactly one
            source; distribution defaults to <b className="text-[#9a8f80]">symlink</b>.
          </>
        ) : (
          <>
            Global prompts span every prompt-capable agent —{" "}
            <b className="text-[#9a8f80]">Generic Agent</b> (
            <span className="font-mono">~/.agents</span>) sits leftmost, then Claude Code
            / CodeX / Copilot / OpenCode / Pi / Qoder. Each row has exactly one source;
            distribution defaults to <b className="text-[#9a8f80]">symlink</b>.
          </>
        )}
      </p>
    </ScreenScroll>
  );
}
