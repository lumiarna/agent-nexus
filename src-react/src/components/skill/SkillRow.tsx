import { useState } from "react";
import type { MouseEvent } from "react";
import { Check, Folder, Globe, Plus } from "lucide-react";
import { AgentLogo } from "@/components/ui/agent-logo";
import { AgentMatrixCells, SourceBadge } from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Modal, ModalHeader } from "@/components/ui/modal";
import { Toggle } from "@/components/ui/toggle";
import { agentColor, targetAgentsOf } from "@/lib/tokens";
import type {
  AgentName,
  PlacementCells,
  ProjectCustomDestinationState,
  ProjectCustomSkillIntent,
  Skill,
} from "@/types";

interface SkillRowProps {
  skill: Skill;
  mode: "global" | "project";
  agents?: AgentName[];
  onToggleAgentCell: (agent: AgentName, event: MouseEvent<HTMLSpanElement>) => void;
  onProjectCustomIntent: (intent: ProjectCustomSkillIntent) => void;
  onToggleDmi: () => void;
  onOpen: () => void;
  onReveal: () => void;
}

function destinationIdentity(target: ProjectCustomDestinationState) {
  return target.kind === "global"
    ? ({ kind: "global" } as const)
    : ({ kind: "project", projectId: target.project.id } as const);
}

function destinationName(target: ProjectCustomDestinationState): string {
  return target.kind === "global" ? "Global" : target.project.name;
}

function hasTarget(cells: PlacementCells): boolean {
  return Object.values(cells).some((role) => role === "target");
}

function PropagateTo({
  skillId,
  targets,
  onIntent,
}: {
  skillId: string;
  targets: ProjectCustomDestinationState[];
  onIntent: (intent: ProjectCustomSkillIntent) => void;
}) {
  const [open, setOpen] = useState(false);
  const enabledCount = targets.filter((target) => hasTarget(target.cells)).length;
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
          skillId={skillId}
          targets={targets}
          onIntent={onIntent}
          onClose={() => setOpen(false)}
        />
      ) : null}
    </div>
  );
}

function PropagateModal({
  skillId,
  targets,
  onIntent,
  onClose,
}: {
  skillId: string;
  targets: ProjectCustomDestinationState[];
  onIntent: (intent: ProjectCustomSkillIntent) => void;
  onClose: () => void;
}) {
  const global = targets.filter((target) => target.kind === "global");
  const projects = targets.filter((target) => target.kind === "project");
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
              onToggle={() =>
                onIntent({
                  kind: "setTargetEnabled",
                  skillId,
                  destination: destinationIdentity(target),
                  enabled: !hasTarget(target.cells),
                })
              }
            />
          ))}
        </ModalSection>
        <ModalSection label="Projects" icon={<Folder size={12} />}>
          {projects.length === 0 ? (
            <div className="px-2 py-2 text-[11px] text-[#b3a999]">
              No effectively active Projects.
            </div>
          ) : (
            projects.map((target) => (
              <ModalTargetRow
                key={target.kind === "project" ? target.project.id : "global"}
                target={target}
                onToggle={() =>
                  onIntent({
                    kind: "setTargetEnabled",
                    skillId,
                    destination: destinationIdentity(target),
                    enabled: !hasTarget(target.cells),
                  })
                }
              />
            ))
          )}
        </ModalSection>
      </div>
      <div className="border-t border-nexus-panel px-5 py-3 text-[10.5px] text-[#b3a999]">
        One target per click · entry Agent is resolved from current Settings · conflicting paths
        are never overwritten
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
  onToggle,
}: {
  target: ProjectCustomDestinationState;
  onToggle: () => void;
}) {
  const enabled = hasTarget(target.cells);
  const targetAgents = targetAgentsOf(target.cells);
  return (
    <div className="flex items-center justify-between gap-3 rounded-[10px] px-2 py-2 hover:bg-nexus-sand">
      <div className="min-w-0">
        <div className="text-[12px] font-semibold text-nexus-ink">
          {destinationName(target)}
        </div>
        <div className="mt-0.5 font-mono text-[10.5px] text-[#a99a89]">
          Managed {target.kind === "global" ? "Global" : "Project"} placement
        </div>
        <div className="mt-1 flex items-center gap-1.5">
          {enabled ? (
            <>
              <span className="text-[10.5px] font-semibold text-nexus-good">On ·</span>
              {targetAgents.map((agent) => (
                <span
                  key={agent}
                  title={agent}
                  className="inline-flex h-[18px] w-[18px] items-center justify-center rounded-[5px]"
                  style={{ background: agentColor(agent) + "26" }}
                >
                  <AgentLogo agent={agent} className="h-[11px] w-[11px]" />
                </span>
              ))}
            </>
          ) : (
            <span className="text-[11px] text-[#b3a999]">Not propagated</span>
          )}
        </div>
      </div>
      <Button
        variant={enabled ? "subtle" : "primary"}
        size="sm"
        className="px-3"
        onClick={onToggle}
      >
        {enabled ? (
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

export function SkillRow({
  skill,
  mode,
  agents,
  onToggleAgentCell,
  onProjectCustomIntent,
  onToggleDmi,
  onOpen,
  onReveal,
}: SkillRowProps) {
  const summary = skill.skill;
  const sourceTooltip =
    skill.kind === "agentCanonical"
      ? undefined
      : `Linked from Project custom source · ${skill.sourceProject.name} · ${summary.path}`;

  let distribution: React.ReactNode;
  switch (skill.kind) {
    case "agentCanonical":
      distribution = (
        <AgentMatrixCells
          cells={skill.cells}
          agents={agents}
          onToggle={onToggleAgentCell}
        />
      );
      break;
    case "projectCustomIncoming":
      distribution = (
        <AgentMatrixCells
          cells={skill.cells}
          agents={agents}
          sourceless
          onToggle={(agent) =>
            onProjectCustomIntent({
              kind: "setAgentPlacement",
              skillId: summary.skillId,
              destination: { kind: "project", projectId: skill.targetProject.id },
              agent,
              enabled: skill.cells[agent] !== "target",
            })
          }
        />
      );
      break;
    case "projectCustomCanonical": {
      if (mode === "project") {
        distribution = (
          <PropagateTo
            skillId={summary.skillId}
            targets={skill.destinations}
            onIntent={onProjectCustomIntent}
          />
        );
      } else {
        const global = skill.destinations.find((target) => target.kind === "global");
        distribution = global ? (
          <AgentMatrixCells
            cells={global.cells}
            agents={agents}
            sourceless
            onToggle={(agent) =>
              onProjectCustomIntent({
                kind: "setAgentPlacement",
                skillId: summary.skillId,
                destination: { kind: "global" },
                agent,
                enabled: global.cells[agent] !== "target",
              })
            }
          />
        ) : null;
      }
      break;
    }
  }

  return (
    <div
      className="grid items-center gap-4 border-t border-[#f3eee5] px-5 py-[13px]"
      style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-[14px] font-bold text-nexus-ink">{summary.name}</span>
          {skill.kind === "agentCanonical" ? (
            <SourceBadge agent={skill.sourceAgent} />
          ) : (
            <SourceBadge label="Project source" title={sourceTooltip} />
          )}
        </div>
        <div className="mt-0.5 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-[#a99a89]">
          {summary.desc}
        </div>
      </div>
      {distribution}
      <div className="flex flex-col items-center gap-1">
        <Toggle checked={summary.disabled} tone="warn" onChange={onToggleDmi} />
        <span className="text-[10px] text-[#b3a999]">{summary.disabled ? "On" : "Off"}</span>
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
