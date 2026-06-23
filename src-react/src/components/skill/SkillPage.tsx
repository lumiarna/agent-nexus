import { useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { MatrixLegend } from "@/components/ui/agent-icon";
import { Card, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { ScreenScroll } from "@/components/shell/screen";
import { skillsApi } from "@/lib/api/skills";
import { nexus } from "@/lib/mock";
import { useProjectsQuery } from "@/lib/query/projects";
import {
  useSetSkillDisabledMutation,
  useSetSkillTargetMutation,
  useSkillsQuery,
} from "@/lib/query/skills";
import { isTauriRuntime } from "@/lib/runtime";
import { toggleCellRole } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { AgentName, Skill } from "@/types";

type Scope = "global" | "project";

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

export function SkillPage() {
  const desktop = isTauriRuntime();
  const skillsQuery = useSkillsQuery();
  const projectsQuery = useProjectsQuery();
  const setSkillTarget = useSetSkillTargetMutation();
  const setSkillDisabled = useSetSkillDisabledMutation();
  const [mockSkills, setMockSkills] = useState(() => nexus.skills());
  const [scope, setScope] = useState<Scope>("global");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const mockProjects = useRef(nexus.projects().filter((p) => p.status === "active"));
  const skills = desktop ? skillsQuery.data ?? [] : mockSkills;
  const projects = desktop
    ? (projectsQuery.data ?? []).filter((p) => p.status === "active")
    : mockProjects.current;
  const queryError =
    desktop && skillsQuery.error ? getErrorMessage(skillsQuery.error) : null;
  const pageError = queryError;
  const isLoading = desktop && skillsQuery.isLoading;
  const isRefreshing = desktop && skillsQuery.isFetching;

  async function scan() {
    if (!desktop) {
      toast("Desktop runtime required for scanning");
      return;
    }

    try {
      const result = await skillsQuery.refetch();
      if (result.data) {
        toast(`Refreshed ${result.data.length} ${result.data.length === 1 ? "skill" : "skills"}`);
      }
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleCell(skill: Skill, agent: AgentName) {
    if (skill.cells[agent] === "source") return;

    if (!desktop) {
      setMockSkills((s) =>
        s.map((k) => (k.id === skill.id ? { ...k, cells: toggleCellRole(k.cells, agent) } : k)),
      );
      return;
    }

    try {
      await setSkillTarget.mutateAsync({
        skillId: skill.id,
        agent,
        enabled: skill.cells[agent] !== "target",
      });
      toast(skill.cells[agent] === "target" ? "Target removed" : "Target linked");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleDmi(skill: Skill) {
    if (!desktop) {
      setMockSkills((s) =>
        s.map((k) => (k.id === skill.id ? { ...k, disabled: !k.disabled } : k)),
      );
      return;
    }

    try {
      await setSkillDisabled.mutateAsync({ id: skill.id, disabled: !skill.disabled });
      toast(!skill.disabled ? "Model invocation disabled" : "Model invocation enabled");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openSource(skill: Skill) {
    if (!desktop) {
      toast(`Open source · ${skill.path}`);
      return;
    }

    try {
      await skillsApi.openSource(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealPath(skill: Skill) {
    if (!desktop) {
      toast(`Reveal in file manager · ${skill.path}`);
      return;
    }

    try {
      await skillsApi.revealPath(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const isProj = scope === "project";
  const q = search.trim().toLowerCase();
  const projectCounts = new Map<string, number>();
  for (const skill of skills) {
    if (skill.scope !== "project" || !skill.projectId) continue;
    projectCounts.set(skill.projectId, (projectCounts.get(skill.projectId) ?? 0) + 1);
  }
  let set = skills.filter((k) =>
    isProj
      ? k.scope === "project" && (projectId === null || k.projectId === projectId)
      : k.scope === "global",
  );
  if (q) set = set.filter((k) => k.name.toLowerCase().includes(q) || k.desc.toLowerCase().includes(q));

  let emptyTitle = "";
  let emptyBody = "";
  if (q && set.length === 0) {
    emptyTitle = "No matching skills";
    emptyBody = `No skill matches “${search}” in this scope.`;
  } else if (isProj) {
    const pn = projectId === null ? "any project" : projects.find((p) => p.id === projectId)?.name;
    emptyTitle = "No project skills";
    emptyBody = `No project-scoped skills recorded for ${pn}.`;
  } else {
    emptyTitle = "No global skills";
    emptyBody = "No global skills discovered across agents yet.";
  }

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
            Skill
          </h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Shared capability assets · Agent Matrix drives distribution
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
          onChange={(e) => setSearch(e.target.value)}
        />
        <MatrixLegend />
      </div>

      {isProj ? (
        <div className="mt-3.5 flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-[#b3a999]">Project</span>
          <Chip active={projectId === null} onClick={() => setProjectId(null)}>
            All
          </Chip>
          {projects.map((p) => (
            <Chip
              key={p.id}
              active={projectId === p.id}
              onClick={() => setProjectId(p.id)}
            >
              <span>{p.name}</span>
              {projectCounts.get(p.id) ? (
                <span className="ml-1 opacity-80">{projectCounts.get(p.id)}</span>
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
          <div>Skill</div>
          <div className="text-center">Distribution</div>
          <div className="text-center">Disable invoke</div>
          <div className="text-right">Source file</div>
        </div>
        {isLoading && set.length === 0 ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">Scanning skills</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">
              Reading global and project skill directories.
            </div>
          </div>
        ) : pageError ? (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-nexus-crit">Skill scan failed</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{pageError}</div>
          </div>
        ) : set.length > 0 ? (
          set.map((k) => (
            <SkillRow
              key={k.id}
              skill={k}
              onToggleCell={(a) => void toggleCell(k, a)}
              onToggleDmi={() => void toggleDmi(k)}
              onOpen={() => void openSource(k)}
              onReveal={() => void revealPath(k)}
            />
          ))
        ) : (
          <div className="px-6 py-12 text-center">
            <div className="text-[14px] font-bold text-[#7a6f60]">{emptyTitle}</div>
            <div className="mt-1.5 text-[12.5px] text-[#b3a999]">{emptyBody}</div>
          </div>
        )}
      </Card>

      <p className="mt-3.5 text-[11.5px] text-[#b3a999]">
        Distribution targets show as Agent icons — <b className="text-[#9a8f80]">Agents</b> (the
        shared <span className="font-mono">~/.agents</span> dir) sits leftmost, then Claude Code /
        CodeX / Copilot / OpenCode. Each row has exactly one source.
      </p>
    </ScreenScroll>
  );
}
