import { useMemo, useState } from "react";
import { Globe, Folder, Check, Plus } from "lucide-react";
import { AgentLogo } from "@/components/ui/agent-logo";
import { AgentMatrixCells, SourceBadge } from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Modal, ModalHeader } from "@/components/ui/modal";
import { Toggle } from "@/components/ui/toggle";
import { agentColor, isProjectCustomSkill, srcAgentOf } from "@/lib/tokens";
import type { AgentName, Skill } from "@/types";
import type { PropagationTarget } from "./propagation";

interface SkillRowProps {
  skill: Skill;
  /** Which page the row is rendered in. In `project` view a Project custom
   *  Skill that is the *source* shows a "Propagate to…" control; an *incoming*
   *  target-Project projection row shows a sourceless Agent Matrix instead. */
  mode: "global" | "project";
  /** Owning/source Project name — used in the Project custom source tooltip for
   *  the source row. For an incoming projection row, pass the *source* Project
   *  name via `sourceProjectName` instead. */
  projectName?: string;
  /** Source Project name for an incoming projection row (the Project the
   *  canonical `Project custom source` lives in). Falls back to `projectName`. */
  sourceProjectName?: string;
  /** Enabled Agents in canonical order — narrows the rendered matrix columns. */
  agents?: AgentName[];
  onToggleCell: (agent: AgentName) => void;
  /** Toggle a single Agent placement inside an incoming target-Project row.
   *  Required when the row is an incoming projection; the canonical
   *  `onToggleCell` is used otherwise. */
  onToggleProjectCell?: (agent: AgentName) => void;
  onToggleDmi: () => void;
  /** Enable Global propagation for a Project custom Skill via the chosen entry Agent. */
  onPropagateGlobal?: (entryAgent: AgentName) => void;
  /** Remove every Global placement for a Project custom Skill. */
  onUnpropagateGlobal?: () => void;
  /** Source-side: propagate to (or cancel) a target Project. */
  onPropagateProject?: (projectId: string, defaultAgent: AgentName) => void;
  onUnpropagateProject?: (projectId: string) => void;
  /** Modal target list for a source Project custom Skill (Global + Projects),
   *  computed by the parent from the full skill list. */
  propagationTargets?: PropagationTarget[];
  onOpen: () => void;
  onReveal: () => void;
}

/** Source-row "Propagate to…" control: a button with an enabled-target count
 *  badge that opens a modal listing Global + other active Projects with their
 *  current state and an Add/Remove action each. One target per click — no
 *  multi-select batch propagation. */
function PropagateTo({
  targets,
  onPropagateGlobal,
  onUnpropagateGlobal,
  onPropagateProject,
  onUnpropagateProject,
}: {
  targets: PropagationTarget[];
  onPropagateGlobal?: (entryAgent: AgentName) => void;
  onUnpropagateGlobal?: () => void;
  onPropagateProject?: (projectId: string, defaultAgent: AgentName) => void;
  onUnpropagateProject?: (projectId: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const enabledCount = targets.filter((t) => t.enabled).length;

  return (
    <div className="relative flex flex-col items-center gap-1.5">
      <button
        onClick={() => setOpen(true)}
        className="inline-flex items-center gap-1.5 rounded-full border border-nexus-border2 bg-white px-3 py-1.5 text-[11.5px] font-semibold text-nexus-ink hover:border-nexus-accent"
      >
        Propagate to…
        <span className="rounded-full bg-nexus-accent/15 px-1.5 text-[10px] font-bold text-nexus-accent">
          {enabledCount}
        </span>
      </button>
      <span className="text-[10px] text-[#b3a999]">
        {enabledCount > 0 ? `${enabledCount} target(s)` : "Not propagated"}
      </span>
      {open ? (
        <PropagateModal
          targets={targets}
          onClose={() => setOpen(false)}
          onPropagateGlobal={onPropagateGlobal}
          onUnpropagateGlobal={onUnpropagateGlobal}
          onPropagateProject={onPropagateProject}
          onUnpropagateProject={onUnpropagateProject}
        />
      ) : null}
    </div>
  );
}

function PropagateModal({
  targets,
  onClose,
  onPropagateGlobal,
  onUnpropagateGlobal,
  onPropagateProject,
  onUnpropagateProject,
}: {
  targets: PropagationTarget[];
  onClose: () => void;
  onPropagateGlobal?: (entryAgent: AgentName) => void;
  onUnpropagateGlobal?: () => void;
  onPropagateProject?: (projectId: string, defaultAgent: AgentName) => void;
  onUnpropagateProject?: (projectId: string) => void;
}) {
  const global = targets.filter((t) => t.kind === "global");
  const projects = targets.filter((t) => t.kind === "project");

  return (
    <Modal open onClose={onClose} className="w-[460px]">
      <ModalHeader
        title="Propagate skill"
        subtitle="Project custom source · managed placement, not a copy"
        onClose={onClose}
      />
      <div className="max-h-[58vh] overflow-y-auto px-3 py-3">
        <ModalSection label="Global" icon={<Globe size={12} />}>
          {global.map((target) => (
            <ModalTargetRow
              key="global"
              target={target}
              pathPreview="~/.agents/skills (Global placement)"
              onToggle={
                target.enabled
                  ? () => onUnpropagateGlobal?.()
                  : () => onPropagateGlobal?.(target.defaultAgent)
              }
            />
          ))}
        </ModalSection>

        <ModalSection label="Projects" icon={<Folder size={12} />}>
          {projects.length === 0 ? (
            <div className="px-2 py-2 text-[11px] text-[#b3a999]">
              No other active Projects.
            </div>
          ) : (
            projects.map((target) => (
              <ModalTargetRow
                key={target.projectId}
                target={target}
                pathPreview={`${target.projectName}/.claude/skills (project placement)`}
                onToggle={
                  target.enabled
                    ? () => onUnpropagateProject?.(target.projectId!)
                    : () => onPropagateProject?.(target.projectId!, target.defaultAgent)
                }              />
            ))
          )}
        </ModalSection>
      </div>
      <div className="border-t border-nexus-panel px-5 py-3 text-[10.5px] text-[#b3a999]">
        One target per click · target Agent uses the Settings default · a target
        path that already exists is not overwritten
      </div>
    </Modal>
  );
}

function ModalSection({
  label,
  icon,
  children,
}: {
  label: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-2">
      <div className="flex items-center gap-1.5 px-2 py-1 text-[9.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]">
        {icon}
        {label}
      </div>
      {children}
    </div>
  );
}

function ModalTargetRow({
  target,
  pathPreview,
  onToggle,
}: {
  target: PropagationTarget;
  pathPreview: string;
  onToggle: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-[10px] px-2 py-2 hover:bg-nexus-sand">
      <div className="min-w-0">
        <div className="text-[12px] font-semibold text-nexus-ink">{target.projectName}</div>
        <div className="mt-0.5 font-mono text-[10.5px] text-[#a99a89]">{pathPreview}</div>
        <div className="mt-1 flex items-center gap-1.5">
          {target.enabled ? (
            <>
              <span className="text-[10.5px] font-semibold text-nexus-good">On ·</span>
              {target.targetAgents.map((a) => (
                <span
                  key={a}
                  title={a}
                  className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px]"
                  style={{ background: agentColor(a) + "26" }}
                >
                  <AgentLogo agent={a} className="h-[11px] w-[11px]" />
                </span>
              ))}
            </>
          ) : (
            <span className="text-[11px] text-[#b3a999]">Not propagated</span>
          )}
        </div>
      </div>
      <Button
        variant={target.enabled ? "subtle" : "primary"}
        size="sm"
        className="px-3"
        onClick={onToggle}
      >
        {target.enabled ? (
          <>
            <Check size={12} /> Remove
          </>
        ) : (
          <>
            <Plus size={12} /> Add
          </>
        )}
      </Button>
    </div>
  );
}

/** One Skill table row — shared by the Skill page and Project detail.
 *  Columns: Skill · Distribution · Disable invoke · Source file. */
export function SkillRow({
  skill,
  mode,
  projectName,
  sourceProjectName,
  agents,
  onToggleCell,
  onToggleProjectCell,
  onToggleDmi,
  onPropagateGlobal,
  onUnpropagateGlobal,
  onPropagateProject,
  onUnpropagateProject,
  propagationTargets,
  onOpen,
  onReveal,
}: SkillRowProps) {
  const isCustom = isProjectCustomSkill(skill);
  const isIncoming = isCustom && skill.placementScope === "project";
  const isSourceCustomRow = isCustom && !isIncoming && mode === "project";
  const sourceTooltip = isCustom
    ? `Linked from Project custom source${
        (sourceProjectName ?? projectName) ? ` · ${sourceProjectName ?? projectName}` : ""
      } · ${skill.path}`
    : undefined;

  const distributionCell = useMemo(() => {
    if (isSourceCustomRow && propagationTargets && propagationTargets.length > 0) {
      return (
        <PropagateTo
          targets={propagationTargets}
          onPropagateGlobal={onPropagateGlobal}
          onUnpropagateGlobal={onUnpropagateGlobal}
          onPropagateProject={onPropagateProject}
          onUnpropagateProject={onUnpropagateProject}
        />
      );
    }
    return (
      <AgentMatrixCells
        cells={skill.cells}
        agents={agents}
        onToggle={isIncoming ? (onToggleProjectCell ?? onToggleCell) : onToggleCell}        sourceless={isCustom}
      />
    );
  }, [
    isSourceCustomRow,
    isIncoming,
    isCustom,
    propagationTargets,
    onPropagateGlobal,
    onUnpropagateGlobal,
    onPropagateProject,
    onUnpropagateProject,
    skill.cells,
    agents,
    onToggleCell,
    onToggleProjectCell,
  ]);

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
      {distributionCell}
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
