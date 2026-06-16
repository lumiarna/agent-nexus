import type { CSSProperties } from "react";
import type { AgentName, CellRole, Cells } from "@/types";
import { AGENT_ORDER, agentAbbr, agentColor } from "@/lib/tokens";
import { cn } from "@/lib/utils";

interface AgentIconProps {
  role: CellRole;
  agent: AgentName;
  onClick?: () => void;
  title?: string;
}

const BASE =
  "inline-flex h-[26px] w-[26px] flex-none items-center justify-center rounded-[8px] text-[9px] font-extrabold tracking-[.02em] transition-all select-none";

/** One Agent Matrix cell: source (filled), target (tinted), none (dashed). */
export function AgentIcon({ role, agent, onClick, title }: AgentIconProps) {
  const col = agentColor(agent);
  let style: CSSProperties = {};
  let cls = "";
  if (role === "source") {
    style = { background: col, color: "#fff", boxShadow: `0 0 0 2px ${col}33` };
    cls = "cursor-default";
  } else if (role === "target") {
    style = { background: col + "22", color: col, border: `1px solid ${col}55` };
    cls = "cursor-pointer";
  } else {
    cls = "cursor-pointer border border-dashed border-[#ddccb6] text-[#cabfae]";
  }
  return (
    <span onClick={onClick} title={title} className={cn(BASE, cls)} style={style}>
      {agentAbbr(agent)}
    </span>
  );
}

/** Small colored abbr pill marking a row's source agent. */
export function SourceBadge({ agent }: { agent: AgentName }) {
  const col = agentColor(agent);
  return (
    <span
      title="Source agent"
      className="inline-flex items-center justify-center rounded-[5px] px-1.5 py-px text-[9px] font-extrabold tracking-[.04em]"
      style={{ background: col + "1c", color: col }}
    >
      {agentAbbr(agent)}
    </span>
  );
}

function cellTitle(agent: AgentName, role: CellRole): string {
  const suffix =
    role === "source"
      ? " · source (fixed)"
      : role === "target"
        ? " · target — click to remove"
        : " · none — click to add target";
  return agent + suffix;
}

/** The row of clickable Agent Matrix cells (canonical agent order). */
export function AgentMatrixCells({
  cells,
  onToggle,
}: {
  cells: Cells;
  onToggle: (agent: AgentName) => void;
}) {
  return (
    <div className="flex justify-center gap-[5px]">
      {AGENT_ORDER.map((a) => (
        <AgentIcon
          key={a}
          agent={a}
          role={cells[a]}
          title={cellTitle(a, cells[a])}
          onClick={() => onToggle(a)}
        />
      ))}
    </div>
  );
}

/** Source / target / none legend shown in Skill & Prompt headers. */
export function MatrixLegend() {
  return (
    <div className="flex items-center gap-3.5 text-[11.5px] text-[#9a8f80]">
      <span className="inline-flex items-center gap-1.5">
        <span className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px] bg-nexus-accent text-[8px] font-extrabold text-white">
          CC
        </span>
        source
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px] bg-[rgba(157,122,100,.18)] text-[8px] font-extrabold text-nexus-accent">
          CX
        </span>
        target
      </span>
      <span className="inline-flex items-center gap-1.5">
        <span className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px] border border-dashed border-[#ddccb6] text-[8px] font-extrabold text-[#cabfae]">
          OC
        </span>
        none
      </span>
    </div>
  );
}
