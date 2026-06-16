import { useState } from "react";
import { toast } from "sonner";
import { MatrixLegend } from "@/components/ui/agent-icon";
import { Card, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { ScreenScroll } from "@/components/shell/screen";
import { nexus } from "@/lib/mock";
import { toggleCellRole } from "@/lib/tokens";
import type { AgentName } from "@/types";

type Scope = "global" | "project";

export function SkillPage() {
  const [skills, setSkills] = useState(() => nexus.skills());
  const [scope, setScope] = useState<Scope>("global");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [projects] = useState(() => nexus.projects().filter((p) => p.status === "active"));

  const toggleCell = (id: string, agent: AgentName) =>
    setSkills((s) =>
      s.map((k) => (k.id === id ? { ...k, cells: toggleCellRole(k.cells, agent) } : k)),
    );
  const toggleDmi = (id: string) =>
    setSkills((s) => s.map((k) => (k.id === id ? { ...k, disabled: !k.disabled } : k)));

  const isProj = scope === "project";
  const q = search.trim().toLowerCase();
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
        <Segmented<Scope>
          options={[
            { value: "global", label: "Global" },
            { value: "project", label: "Project" },
          ]}
          value={scope}
          onChange={setScope}
        />
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
              {p.name}
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
        {set.length > 0 ? (
          set.map((k) => (
            <SkillRow
              key={k.id}
              skill={k}
              onToggleCell={(a) => toggleCell(k.id, a)}
              onToggleDmi={() => toggleDmi(k.id)}
              onOpen={() => toast(`Open source · ${k.path}`)}
              onReveal={() => toast(`Reveal in file manager · ${k.path}`)}
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
