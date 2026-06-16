import { AgentMatrixCells, SourceBadge } from "@/components/ui/agent-icon";
import { Toggle } from "@/components/ui/toggle";
import { srcAgentOf } from "@/lib/tokens";
import type { AgentName, Skill } from "@/types";

interface SkillRowProps {
  skill: Skill;
  onToggleCell: (agent: AgentName) => void;
  onToggleDmi: () => void;
  onOpen: () => void;
  onReveal: () => void;
}

/** One Skill table row — shared by the Skill page and Project detail.
 *  Columns: Skill · Distribution · Disable invoke · Source file. */
export function SkillRow({
  skill,
  onToggleCell,
  onToggleDmi,
  onOpen,
  onReveal,
}: SkillRowProps) {
  return (
    <div
      className="grid items-center gap-4 border-t border-[#f3eee5] px-5 py-[13px]"
      style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-[14px] font-bold text-nexus-ink">{skill.name}</span>
          <SourceBadge agent={srcAgentOf(skill.cells)} />
        </div>
        <div className="mt-0.5 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-[#a99a89]">
          {skill.desc}
        </div>
      </div>
      <AgentMatrixCells cells={skill.cells} onToggle={onToggleCell} />
      <div className="flex flex-col items-center gap-1">
        <Toggle checked={skill.disabled} tone="warn" onChange={onToggleDmi} />
        <span className="text-[10px] text-[#b3a999]">{skill.disabled ? "On" : "Off"}</span>
      </div>
      <div className="flex flex-col items-end gap-[5px]">
        <span
          onClick={onOpen}
          className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-nexus-accent hover:underline"
        >
          Open source
        </span>
        <span
          onClick={onReveal}
          className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-[#a99a89] hover:underline"
        >
          Reveal path
        </span>
      </div>
    </div>
  );
}
