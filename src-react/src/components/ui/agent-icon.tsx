import type { CSSProperties } from "react";
import type { AgentName, CellRole, Cells } from "@/types";
import { AGENT_ORDER, agentColor } from "@/lib/tokens";
import { AgentLogo } from "@/components/ui/agent-logo";
import { cn } from "@/lib/utils";

interface AgentIconProps {
  role: CellRole;
  agent: AgentName;
  onClick?: () => void;
  title?: string;
}

const BASE =
  "inline-flex h-[26px] w-[26px] flex-none items-center justify-center rounded-[8px] bg-white transition-all select-none";

/** Container style for a matrix cell by role. Logos keep their brand color;
 *  state is carried by the chip — tinted ring (source), thin ring (target),
 *  dashed + desaturated (none). */
function cellStyle(col: string, role: CellRole): { style: CSSProperties; cls: string } {
  if (role === "source") {
    return {
      style: { background: col + "1a", boxShadow: `inset 0 0 0 1.5px ${col}` },
      cls: "cursor-default",
    };
  }
  if (role === "target") {
    return {
      style: { boxShadow: `inset 0 0 0 1px ${col}59` },
      cls: "cursor-pointer hover:brightness-95",
    };
  }
  return {
    style: { border: "1px dashed #ddccb6" },
    cls: "cursor-pointer opacity-50 grayscale hover:opacity-80",
  };
}

/** One Agent Matrix cell: source (filled ring), target (thin ring), none (dashed). */
export function AgentIcon({ role, agent, onClick, title }: AgentIconProps) {
  const { style, cls } = cellStyle(agentColor(agent), role);
  return (
    <span onClick={onClick} title={title} className={cn(BASE, cls)} style={style}>
      <AgentLogo agent={agent} className="h-[15px] w-[15px]" />
    </span>
  );
}

/** Small chip marking a row's source agent. */
export function SourceBadge({ agent }: { agent: AgentName }) {
  const col = agentColor(agent);
  return (
    <span
      title="Source agent"
      className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px]"
      style={{ background: col + "1c", boxShadow: `inset 0 0 0 1px ${col}40` }}
    >
      <AgentLogo agent={agent} className="h-[11px] w-[11px]" />
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

/** A single legend chip — a non-interactive cell in a given role. */
function LegendChip({ agent, role, label }: { agent: AgentName; role: CellRole; label: string }) {
  const { style } = cellStyle(agentColor(agent), role);
  return (
    <span className="inline-flex items-center gap-1.5">
      <span
        className={cn(
          "inline-flex h-[20px] w-[20px] items-center justify-center rounded-[6px] bg-white",
          role === "none" && "opacity-50 grayscale",
        )}
        style={style}
      >
        <AgentLogo agent={agent} className="h-[12px] w-[12px]" />
      </span>
      {label}
    </span>
  );
}

/** Source / target / none legend shown in Skill & Prompt headers. */
export function MatrixLegend() {
  return (
    <div className="flex items-center gap-3.5 text-[11.5px] text-[#9a8f80]">
      <LegendChip agent="Claude Code" role="source" label="source" />
      <LegendChip agent="CodeX" role="target" label="target" />
      <LegendChip agent="OpenCode" role="none" label="none" />
    </div>
  );
}
