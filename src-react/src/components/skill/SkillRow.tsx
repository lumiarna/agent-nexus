import { AgentLogo } from "@/components/ui/agent-logo";
import { AgentMatrixCells, SourceBadge } from "@/components/ui/agent-icon";
import { Toggle } from "@/components/ui/toggle";
import {
  agentColor,
  hasGlobalPlacement,
  isProjectCustomSkill,
  srcAgentOf,
  targetAgentsOf,
} from "@/lib/tokens";
import type { AgentName, Skill } from "@/types";

interface SkillRowProps {
  skill: Skill;
  /** Which page the row is rendered in. In `project` view a Project custom Skill
   *  hides the Agent Matrix and shows a Propagate-to-Global control instead; in
   *  `global` view it shows the Global placement cells (target / none, no source). */
  mode: "global" | "project";
  /** Owning Project name — used in the Project custom source tooltip. */
  projectName?: string;
  onToggleCell: (agent: AgentName) => void;
  onToggleDmi: () => void;
  /** Enable Global propagation for a Project custom Skill via the chosen entry Agent. */
  onPropagateGlobal?: (entryAgent: AgentName) => void;
  /** Remove every Global placement for a Project custom Skill. */
  onUnpropagateGlobal?: () => void;
  onOpen: () => void;
  onReveal: () => void;
}

/** Entry Agent default when first propagating a Project custom Skill to Global.
 *  Generic Agent is the most source-neutral landing spot (matrix leftmost). */
const DEFAULT_GLOBAL_ENTRY: AgentName = "Generic Agent";

/** Distribution control for a Project custom Skill inside the Project view:
 *  a single Propagate-to-Global toggle. Further fan-out to other Agents happens
 *  on the Global Skill page. */
function PropagateToGlobal({
  skill,
  onPropagate,
  onUnpropagate,
}: {
  skill: Skill;
  onPropagate?: (entryAgent: AgentName) => void;
  onUnpropagate?: () => void;
}) {
  const propagated = hasGlobalPlacement(skill.cells);
  const targets = targetAgentsOf(skill.cells);

  return (
    <div className="flex flex-col items-center gap-1.5">
      <div className="flex items-center gap-2">
        <Toggle
          checked={propagated}
          title={propagated ? "Remove Global placements" : "Propagate to Global"}
          onChange={() => {
            if (propagated) onUnpropagate?.();
            else onPropagate?.(DEFAULT_GLOBAL_ENTRY);
          }}
        />
        <span className="text-[10px] text-[#b3a999]">
          {propagated ? "On Global" : "Propagate"}
        </span>
      </div>
      {propagated && targets.length > 0 ? (
        <div className="flex items-center gap-[3px]">
          {targets.map((a) => (
            <span
              key={a}
              title={`Global placement · ${a}`}
              className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px]"
              style={{ background: agentColor(a) + "26" }}
            >
              <AgentLogo agent={a} className="h-[11px] w-[11px]" />
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

/** One Skill table row — shared by the Skill page and Project detail.
 *  Columns: Skill · Distribution · Disable invoke · Source file. */
export function SkillRow({
  skill,
  mode,
  projectName,
  onToggleCell,
  onToggleDmi,
  onPropagateGlobal,
  onUnpropagateGlobal,
  onOpen,
  onReveal,
}: SkillRowProps) {
  const isCustom = isProjectCustomSkill(skill);
  const sourceTooltip = isCustom
    ? `Linked from Project custom source${projectName ? ` · ${projectName}` : ""} · ${skill.path}`
    : undefined;

  return (
    <div
      className="grid items-center gap-4 border-t border-[#f3eee5] px-5 py-[13px]"
      style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-[14px] font-bold text-nexus-ink">{skill.name}</span>
          {isCustom ? (
            <SourceBadge label="Project source" title={sourceTooltip} />
          ) : (
            <SourceBadge agent={skill.sourceAgent ?? srcAgentOf(skill.cells)} />
          )}
        </div>
        <div className="mt-0.5 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-[#a99a89]">
          {skill.desc}
        </div>
      </div>
      {isCustom && mode === "project" ? (
        <PropagateToGlobal
          skill={skill}
          onPropagate={onPropagateGlobal}
          onUnpropagate={onUnpropagateGlobal}
        />
      ) : (
        <AgentMatrixCells
          cells={skill.cells}
          onToggle={onToggleCell}
          sourceless={isCustom}
        />
      )}
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
