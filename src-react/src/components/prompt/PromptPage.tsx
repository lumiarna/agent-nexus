import { useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import {
  AgentMatrixCells,
  MatrixLegend,
  SourceBadge,
} from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/primitives";
import { ScreenScroll } from "@/components/shell/screen";
import { promptsApi } from "@/lib/api/prompts";
import { nexus } from "@/lib/mock";
import { usePromptsQuery, useSetPromptTargetMutation } from "@/lib/query/prompts";
import { isTauriRuntime } from "@/lib/runtime";
import { srcAgentOf, toggleCellRole } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { AgentName, Prompt } from "@/types";

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
  const setPromptTarget = useSetPromptTargetMutation();
  const [mockPrompts, setMockPrompts] = useState(() => nexus.prompts());
  const prompts = desktop ? promptsQuery.data ?? [] : mockPrompts;
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

  async function toggleCell(prompt: Prompt, agent: AgentName) {
    if (prompt.cells[agent] === "source") return;

    if (!desktop) {
      setMockPrompts((ps) =>
        ps.map((p) => (p.id === prompt.id ? { ...p, cells: toggleCellRole(p.cells, agent) } : p)),
      );
      return;
    }

    try {
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

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
            Prompt
          </h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Global prompt assets · Agent Matrix drives distribution
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
          <MatrixLegend />
        </div>
      </div>

      <Card className="mt-4 overflow-hidden">
        <div
          className="grid items-center gap-4 border-b border-nexus-panel bg-nexus-sand px-5 py-3 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
          style={{ gridTemplateColumns: "1fr 196px 132px" }}
        >
          <div>Prompt</div>
          <div className="text-center">Distribution</div>
          <div className="text-right">Source file</div>
        </div>
        {isLoading && prompts.length === 0 ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">Scanning prompts</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">
              Reading global prompt files from agent capability surfaces.
            </div>
          </div>
        ) : pageError ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-nexus-crit">Prompt scan failed</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{pageError}</div>
          </div>
        ) : prompts.length > 0 ? (
          prompts.map((p) => (
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
              <AgentMatrixCells cells={p.cells} onToggle={(a) => void toggleCell(p, a)} />
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
            <div className="text-[14px] font-bold text-[#7a6f60]">No global prompts</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">
              No global prompt files discovered across prompt-capable agents yet.
            </div>
          </div>
        )}
      </Card>

      <p className="mt-3.5 text-[11.5px] text-[#b3a999]">
        MVP keeps Prompt as a global single-file asset — there is no project-level prompt.
        Distribution defaults to <b className="text-[#9a8f80]">symlink</b>; target paths are
        computed per agent. <b className="text-[#9a8f80]">Agents</b> (
        <span className="font-mono">~/.agents</span>) is the leftmost generic target.
      </p>
    </ScreenScroll>
  );
}
