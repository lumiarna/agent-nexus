import { useState } from "react";
import type { MouseEvent } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { MatrixLegend, isMoveSourceModifier } from "@/components/ui/agent-icon";
import { Card, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { ScreenScroll } from "@/components/shell/screen";
import { useProjectCustomSkillIntent } from "@/components/skill/useProjectCustomSkillIntent";
import {
  projectForSkillRow,
  showsInGlobalSkillPage,
} from "@/components/skill/visibility";
import { skillsApi } from "@/lib/api/skills";
import {
  useMoveSkillSourceMutation,
  useScanSkillsMutation,
  useSetSkillDisabledMutation,
  useSetSkillTargetMutation,
  useSkillsQuery,
} from "@/lib/query/skills";
import { useDisabledAgents, useEnabledAgents } from "@/lib/query/agentPreferences";
import { isTauriRuntime } from "@/lib/runtime";
import { cn } from "@/lib/utils";
import type { AgentName, Skill } from "@/types";

type Scope = "global" | "project";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return "Unexpected error";
}

export function SkillPage() {
  const desktop = isTauriRuntime();
  const skillsQuery = useSkillsQuery();
  const scanSkills = useScanSkillsMutation();
  const setSkillTarget = useSetSkillTargetMutation();
  const moveSkillSource = useMoveSkillSourceMutation();
  const setSkillDisabled = useSetSkillDisabledMutation();
  const applyProjectCustomIntent = useProjectCustomSkillIntent(desktop);
  const enabledAgents = useEnabledAgents();
  const disabledAgents = useDisabledAgents();
  const [scope, setScope] = useState<Scope>("global");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const skills = skillsQuery.data ?? [];
  const queryError = desktop && skillsQuery.error ? getErrorMessage(skillsQuery.error) : null;
  const isLoading = desktop && skillsQuery.isLoading;
  const isRefreshing = desktop && scanSkills.isPending;

  const projectMap = new Map<string, string>();
  for (const row of skills) {
    const project = projectForSkillRow(row);
    if (project) projectMap.set(project.id, project.name);
    if (row.kind === "projectCustomCanonical") {
      for (const target of row.destinations) {
        if (target.kind === "project") projectMap.set(target.project.id, target.project.name);
      }
    }
  }
  const projects = [...projectMap].map(([id, name]) => ({ id, name }));

  async function scan() {
    if (!desktop) {
      toast("Desktop runtime required for scanning");
      return;
    }
    try {
      const skills = await scanSkills.mutateAsync();
      toast(`Refreshed ${skills.length} ${skills.length === 1 ? "skill" : "skills"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleAgentCell(
    skill: Skill,
    agent: AgentName,
    event: MouseEvent<HTMLSpanElement>,
  ) {
    if (skill.kind !== "agentCanonical" || skill.cells[agent] === "source") return;
    if (!desktop) {
      toast("Desktop runtime required for changing skill targets");
      return;
    }
    try {
      if (isMoveSourceModifier(event)) {
        await moveSkillSource.mutateAsync({ skillId: skill.skill.skillId, agent });
        toast(`Source moved to ${agent}`);
      } else {
        await setSkillTarget.mutateAsync({
          skillId: skill.skill.skillId,
          agent,
          enabled: skill.cells[agent] !== "target",
        });
        toast(skill.cells[agent] === "target" ? "Target removed" : "Target linked");
      }
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleDmi(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for changing skill settings");
      return;
    }
    try {
      await setSkillDisabled.mutateAsync({
        id: skill.skill.skillId,
        disabled: !skill.skill.disabled,
      });
      toast(!skill.skill.disabled ? "Model invocation disabled" : "Model invocation enabled");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openSource(skill: Skill) {
    if (!desktop) {
      toast(`Open source · ${skill.skill.path}`);
      return;
    }
    try {
      await skillsApi.openSource(skill.skill.skillId);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealPath(skill: Skill) {
    if (!desktop) {
      toast(`Reveal in file manager · ${skill.skill.path}`);
      return;
    }
    try {
      await skillsApi.revealPath(skill.skill.skillId);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const isProject = scope === "project";
  const query = search.trim().toLowerCase();
  const projectCounts = new Map<string, number>();
  for (const skill of skills) {
    const project = projectForSkillRow(skill);
    if (project) projectCounts.set(project.id, (projectCounts.get(project.id) ?? 0) + 1);
  }
  let visible = skills.filter((skill) => {
    if (!isProject) return showsInGlobalSkillPage(skill);
    const project = projectForSkillRow(skill);
    return project && (projectId === null || project.id === projectId);
  });
  if (query) {
    visible = visible.filter(
      (row) =>
        row.skill.name.toLowerCase().includes(query) ||
        row.skill.desc.toLowerCase().includes(query),
    );
  }
  visible = visible.filter(
    (row) => row.kind !== "agentCanonical" || !disabledAgents.has(row.sourceAgent),
  );

  const emptyTitle = query
    ? "No matching skills"
    : isProject
      ? "No project skills"
      : "No global skills";
  const emptyBody = query
    ? `No skill matches “${search}” in this scope.`
    : isProject
      ? "No project-scoped skills recorded for this selection."
      : "No global skills discovered across agents yet.";

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">Skill</h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Shared capability assets · Agent Matrix drives distribution
          </p>
        </div>
        <div className="flex items-center gap-2.5">
          <Button variant="subtle" size="sm" onClick={() => void scan()} disabled={isRefreshing}>
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
          placeholder="Search by name or description"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <MatrixLegend />
      </div>

      {isProject ? (
        <div className="mt-3.5 flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-[#b3a999]">Project</span>
          <Chip active={projectId === null} onClick={() => setProjectId(null)}>All</Chip>
          {projects.map((project) => (
            <Chip
              key={project.id}
              active={projectId === project.id}
              onClick={() => setProjectId(project.id)}
            >
              <span>{project.name}</span>
              {projectCounts.get(project.id) ? (
                <span className="ml-1 opacity-80">{projectCounts.get(project.id)}</span>
              ) : null}
            </Chip>
          ))}
        </div>
      ) : null}

      <Card className="mt-4 overflow-hidden">
        <div
          className="grid items-center gap-4 border-b border-nexus-panel bg-nexus-sand px-5 py-3 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
          style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
        >
          <div>Skill</div><div className="text-center">Distribution</div>
          <div className="text-center">Disable invoke</div><div className="text-right">Source file</div>
        </div>
        {isLoading && visible.length === 0 ? (
          <div className="px-6 py-12 text-center text-[12.5px] text-[#b3a999]">Scanning skills</div>
        ) : queryError ? (
          <div className="px-6 py-12 text-center text-[12.5px] text-nexus-crit">{queryError}</div>
        ) : visible.length > 0 ? (
          visible.map((skill) => (
            <SkillRow
              key={skill.rowKey}
              skill={skill}
              mode={isProject ? "project" : "global"}
              agents={enabledAgents}
              onToggleAgentCell={(agent, event) => void toggleAgentCell(skill, agent, event)}
              onProjectCustomIntent={(intent) => void applyProjectCustomIntent(intent)}
              onToggleDmi={() => void toggleDmi(skill)}
              onOpen={() => void openSource(skill)}
              onReveal={() => void revealPath(skill)}
            />
          ))
        ) : (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">{emptyTitle}</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{emptyBody}</div>
          </div>
        )}
      </Card>
    </ScreenScroll>
  );
}
