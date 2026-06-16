import type { CSSProperties } from "react";
import type { AgentName, CellRole } from "@/types";
import { agentAbbr, agentColor } from "@/lib/tokens";
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
