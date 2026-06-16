import { useState } from "react";
import { toast } from "sonner";
import {
  AgentMatrixCells,
  MatrixLegend,
  SourceBadge,
} from "@/components/ui/agent-icon";
import { Card } from "@/components/ui/primitives";
import { ScreenScroll } from "@/components/shell/screen";
import { nexus } from "@/lib/mock";
import { srcAgentOf, toggleCellRole } from "@/lib/tokens";
import type { AgentName } from "@/types";

export function PromptPage() {
  const [prompts, setPrompts] = useState(() => nexus.prompts());

  const toggleCell = (id: string, agent: AgentName) =>
    setPrompts((ps) =>
      ps.map((p) => (p.id === id ? { ...p, cells: toggleCellRole(p.cells, agent) } : p)),
    );

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
        <MatrixLegend />
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
        {prompts.map((p) => (
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
            <AgentMatrixCells cells={p.cells} onToggle={(a) => toggleCell(p.id, a)} />
            <div className="flex flex-col items-end gap-[5px]">
              <span
                onClick={() => toast(`Open source · ${p.path}`)}
                className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-nexus-accent hover:underline"
              >
                Open source
              </span>
              <span
                onClick={() => toast(`Reveal in file manager · ${p.path}`)}
                className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-[#a99a89] hover:underline"
              >
                Reveal path
              </span>
            </div>
          </div>
        ))}
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
