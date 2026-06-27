import { useEffect, useState, type ReactNode } from "react";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import {
  AgentMatrixCells,
  MatrixLegend,
  SourceBadge,
} from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Card, Dot } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { SingleValueConfigModal } from "@/components/project/SingleValueConfigModal";
import { StringListConfigModal } from "@/components/project/StringListConfigModal";
import { ScreenScroll } from "@/components/shell/screen";
import { AGENTS } from "@/config/agents";
import { promptsApi } from "@/lib/api/prompts";
import { skillsApi } from "@/lib/api/skills";
import { useNav } from "@/lib/nav";
import { isTauriRuntime } from "@/lib/runtime";
import {
  useDeleteProjectMutation,
  useGitBaseFoldersQuery,
  useProjectsQuery,
  useRecordGitBaseFolderMutation,
  useRecordProjectMutation,
  useRecordProjectsMutation,
  useReorderProjectsMutation,
  useRemoveGitBaseFolderMutation,
  useScanGitBaseFoldersMutation,
  useSetProjectCustomSkillsDirsMutation,
  useSetProjectExtraPromptFilesMutation,
  useSetProjectSessionsDirMutation,
} from "@/lib/query/projects";
import { usePromptsQuery, useSetPromptTargetMutation } from "@/lib/query/prompts";
import {
  useCloudSessionsQuery,
  useLocalSessionsQuery,
} from "@/lib/query/sessions";
import {
  useSetSkillDisabledMutation,
  useSetSkillTargetMutation,
  useSkillsQuery,
} from "@/lib/query/skills";
import { useSessionBackupsQuery } from "@/lib/query/sync";
import { palette, srcAgentOf, targetAgentsOf } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { AgentName, Project, Prompt, Skill, TaskStatus } from "@/types";

// drag | key+path | skill | prompt | session | ⋯
const LIST_COLS = "20px 1.6fr 1fr 1fr 1fr 36px";
type DetailSource = "local" | "cloud";

/** Project prompts collapse to the repo-root files owned by prompt-capable
 *  agents — Generic Agent (AGENTS.md) and Claude Code (CLAUDE.md). */
const PROJECT_PROMPT_AGENTS: AgentName[] = AGENTS.filter(
  (agent) => agent.projectPromptFile,
).map((agent) => agent.name);

/** Per-agent glob an Extra Prompt File must match (e.g. `AGENTS.md` → `AGENTS*.md`). */
const PROMPT_FILE_GLOBS = AGENTS.filter((agent) => agent.projectPromptFile).map(
  (agent) => {
    const file = agent.projectPromptFile as string;
    const stem = file.replace(/\.md$/i, "");
    return { agent: agent.name, glob: `${stem}*.md`, re: new RegExp(`^${stem}.*\\.md$`, "i") };
  },
);

/** True when the basename of `file` matches an Agent prompt-file glob. */
function matchesPromptGlob(file: string): boolean {
  const base = file.trim().replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
  return PROMPT_FILE_GLOBS.some((g) => g.re.test(base));
}

interface MenuState {
  id: string;
  y: number;
  right: number;
}

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

function deriveProjectKey(path: string): string {
  const parts = path
    .trim()
    .replace(/[\\/]+$/, "")
    .split(/[\\/]/)
    .filter(Boolean);
  return parts[parts.length - 1] ?? "";
}

function moveProjectOrder(order: string[], fromId: string, toId: string): string[] | null {
  if (!fromId || fromId === toId) return null;
  const fromIndex = order.indexOf(fromId);
  const toIndex = order.indexOf(toId);
  if (fromIndex < 0 || toIndex < 0) return null;
  return arrayMove(order, fromIndex, toIndex);
}

function taskStatusSummary(status: TaskStatus): { label: string; fg: string; dot: string } {
  if (status === "ok") return { label: "OK", fg: "#5f7a3e", dot: palette.good };
  if (status === "pending") return { label: "Pending", fg: "#9a6f0a", dot: palette.warn };
  if (status === "failed") return { label: "Failed", fg: palette.crit, dot: palette.crit };
  if (status === "skipped") return { label: "Skipped", fg: "#8a7a68", dot: "#c6b6a1" };
  return { label: "Never", fg: "#a99a89", dot: "#d9c9b3" };
}

interface SortableProjectRowProps {
  id: string;
  onClick?: () => void;
  stale?: boolean;
  children: ReactNode;
}

function SortableProjectRow({ id, onClick, stale, children }: SortableProjectRowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });
  return (
    <div
      ref={setNodeRef}
      onClick={onClick}
      className={cn(
        "grid items-center gap-4 border-b border-[#f3eee5] px-5 py-[13px]",
        !stale && "cursor-pointer hover:bg-[#fbf6ef]",
        stale && "bg-[#faf3e8]",
        isDragging && "opacity-50",
      )}
      style={{
        gridTemplateColumns: LIST_COLS,
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      {...attributes}
    >
      <div
        ref={setActivatorNodeRef}
        {...listeners}
        className={cn(
          "flex cursor-grab items-center justify-center text-[11px] tracking-[1px]",
          stale ? "text-[#d9ccb8]" : "text-[#d0c4b4]",
        )}
        title="Drag to reorder"
      >
        ⋮⋮
      </div>
      {children}
    </div>
  );
}

/** Default Session Directory leaf — anything else is a project-level override. */
const DEFAULT_SESSIONS_DIR = "__sessions";

/** A list asset cell: a count chip with up to two lines of small detail beside
 *  it, then `+N` when the detail overflows. */
function AssetCell({ n, lines }: { n: number; lines: string[] }) {
  const shown = lines.slice(0, 2);
  const extra = lines.length - shown.length;
  return (
    <div className="flex min-w-0 items-center gap-2">
      <span className="flex-none rounded-[7px] bg-nexus-bg px-[9px] py-[5px] text-[12px] font-extrabold text-nexus-body">
        {n}
      </span>
      {shown.length > 0 || extra > 0 ? (
        <div className="flex min-w-0 flex-col">
          {shown.map((line) => (
            <span
              key={line}
              className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10.5px] text-[#b3a999]"
            >
              {line}
            </span>
          ))}
          {extra > 0 ? <span className="text-[10px] text-[#c3b9a8]">+{extra}</span> : null}
        </div>
      ) : null}
    </div>
  );
}

export function ProjectPage({ initialProjectId }: { initialProjectId?: string }) {
  const { go } = useNav();
  const desktop = isTauriRuntime();
  const projectsQuery = useProjectsQuery();
  const baseFoldersQuery = useGitBaseFoldersQuery();
  const skillsQuery = useSkillsQuery();
  const promptsQuery = usePromptsQuery();
  const localSessionsQuery = useLocalSessionsQuery();
  const cloudSessionsQuery = useCloudSessionsQuery();
  const sessionBackupsQuery = useSessionBackupsQuery();
  const recordProject = useRecordProjectMutation();
  const recordProjects = useRecordProjectsMutation();
  const deleteProject = useDeleteProjectMutation();
  const reorderProjects = useReorderProjectsMutation();
  const setSkillTarget = useSetSkillTargetMutation();
  const setSkillDisabled = useSetSkillDisabledMutation();
  const setPromptTarget = useSetPromptTargetMutation();
  const setCustomSkillsDirs = useSetProjectCustomSkillsDirsMutation();
  const setExtraPromptFiles = useSetProjectExtraPromptFilesMutation();
  const setSessionsDir = useSetProjectSessionsDirMutation();
  const recordBaseFolder = useRecordGitBaseFolderMutation();
  const removeBaseFolder = useRemoveGitBaseFolderMutation();
  const scanBaseFolders = useScanGitBaseFoldersMutation();
  const projects = projectsQuery.data ?? [];
  const skills = skillsQuery.data ?? [];
  const prompts = promptsQuery.data ?? [];
  const baseFolders = baseFoldersQuery.data ?? [];
  const projectError = projectsQuery.error ? getErrorMessage(projectsQuery.error) : null;
  const baseFoldersError = baseFoldersQuery.error
    ? getErrorMessage(baseFoldersQuery.error)
    : null;
  const [order, setOrder] = useState<string[]>([]);
  const [screen, setScreen] = useState<"list" | "detail">(
    initialProjectId ? "detail" : "list",
  );
  const [detailId, setDetailId] = useState(initialProjectId ?? "");
  const [detailSource, setDetailSource] = useState<DetailSource>("local");
  const [hiddenIds, setHiddenIds] = useState<string[]>([]);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [baseFoldersOpen, setBaseFoldersOpen] = useState(false);
  const [baseFolderPath, setBaseFolderPath] = useState("");
  const [addOpen, setAddOpen] = useState(false);
  const [hasScanned, setHasScanned] = useState(false);
  const [scanSel, setScanSel] = useState<Record<string, boolean>>({});
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [deleteAck, setDeleteAck] = useState(false);
  const [addPath, setAddPath] = useState("");
  const [customDirsOpen, setCustomDirsOpen] = useState(false);
  const [extraFilesOpen, setExtraFilesOpen] = useState(false);
  const [sessionDirOpen, setSessionDirOpen] = useState(false);
  const addKey = deriveProjectKey(addPath);

  useEffect(() => {
    setOrder((current) => {
      const ids = projects.map((p) => p.id);
      const known = current.filter((id) => ids.includes(id));
      const added = ids.filter((id) => !known.includes(id));
      return [...known, ...added];
    });
  }, [projects]);

  const hidden = (id: string) => hiddenIds.includes(id);
  const byId = Object.fromEntries(projects.map((p) => [p.id, p]));
  const ordered = order.map((id) => byId[id]).filter(Boolean) as Project[];
  const visibleProjects = ordered.filter((p) => p.status !== "hidden" && !hidden(p.id));
  const hiddenP = ordered.filter((p) => p.status === "hidden" || hidden(p.id));

  const menuProject = menu ? projects.find((p) => p.id === menu.id) : null;
  const del = deleteId ? projects.find((p) => p.id === deleteId) ?? null : null;

  function openMenu(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({ id, y: r.bottom + 4, right: Math.max(16, window.innerWidth - r.right) });
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  const isRefreshing = projectsQuery.isFetching || baseFoldersQuery.isFetching;

  async function handleRefresh() {
    if (!desktop) {
      toast("Desktop runtime required for refreshing");
      return;
    }
    try {
      const [projectsResult] = await Promise.all([
        projectsQuery.refetch(),
        baseFoldersQuery.refetch(),
      ]);
      const count = projectsResult.data?.length ?? 0;
      toast(`Refreshed ${count} ${count === 1 ? "project" : "projects"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function handleDragEnd(e: DragEndEvent) {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const previousOrder = order;
    const nextOrder = moveProjectOrder(previousOrder, String(active.id), String(over.id));
    if (!nextOrder) return;

    setOrder(nextOrder);
    try {
      await reorderProjects.mutateAsync(nextOrder);
      toast("Project order saved");
    } catch (error) {
      setOrder(previousOrder);
      toast(getErrorMessage(error));
    }
  }

  // Scan modal derived state
  const scanRes = hasScanned ? scanBaseFolders.data ?? [] : [];
  const newCount = scanRes.filter((r) => r.state === "new").length;
  const selCount = scanRes.filter((r) => r.state === "new" && scanSel[r.path]).length;

  // Detail derived state
  const dp = projects.find((p) => p.id === detailId) ?? projects[0];
  const dpSkills = dp ? skills.filter((k) => k.scope === "project" && k.projectId === dp.id) : [];
  const dpPrompts = dp
    ? prompts.filter((p) => p.scope === "project" && p.projectId === dp.id)
    : [];
  const detailSessionsQuery = detailSource === "local" ? localSessionsQuery : cloudSessionsQuery;
  const detailSessions = detailSessionsQuery.data ?? [];
  const dpSessions = dp
    ? detailSessions.filter(
        (session) =>
          session.project === dp.id &&
          (session.source === detailSource || session.source === "both"),
      )
    : [];
  const sessionBackup = dp
    ? (sessionBackupsQuery.data ?? []).find((backup) => backup.projectKey === dp.key)
    : undefined;
  const skillError = skillsQuery.error ? getErrorMessage(skillsQuery.error) : null;
  const promptError = promptsQuery.error ? getErrorMessage(promptsQuery.error) : null;
  const detailSessionError = detailSessionsQuery.error
    ? getErrorMessage(detailSessionsQuery.error)
    : null;
  const sessionBackupSummary = sessionBackup
    ? taskStatusSummary(sessionBackup.task.status)
    : sessionBackupsQuery.error
      ? { label: "Error", fg: palette.crit, dot: palette.crit }
      : sessionBackupsQuery.isLoading
        ? { label: "Loading", fg: "#9a6f0a", dot: palette.warn }
        : { label: "None", fg: "#a99a89", dot: "#d9c9b3" };

  async function toggleCell(skill: Skill, agent: AgentName) {
    if (skill.cells[agent] === "source") return;
    if (!desktop) {
      toast("Desktop runtime required for changing skill targets");
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

  async function propagateGlobal(skill: Skill, entryAgent: AgentName) {
    if (!desktop) {
      toast("Desktop runtime required for propagating skills");
      return;
    }

    try {
      await setSkillTarget.mutateAsync({ skillId: skill.id, agent: entryAgent, enabled: true });
      toast(`Propagated to Global · ${entryAgent}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function unpropagateGlobal(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for changing skill placements");
      return;
    }

    try {
      for (const agent of targetAgentsOf(skill.cells)) {
        await setSkillTarget.mutateAsync({ skillId: skill.id, agent, enabled: false });
      }
      toast("Removed from Global");
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
      await setSkillDisabled.mutateAsync({ id: skill.id, disabled: !skill.disabled });
      toast(!skill.disabled ? "Model invocation disabled" : "Model invocation enabled");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function togglePromptCell(prompt: Prompt, agent: AgentName) {
    if (prompt.cells[agent] === "source") return;
    if (!desktop) {
      toast("Desktop runtime required for changing prompt targets");
      return;
    }

    try {
      await setPromptTarget.mutateAsync({
        promptId: prompt.id,
        agent,
        enabled: prompt.cells[agent] !== "target",
      });
      toast(prompt.cells[agent] === "target" ? "Target removed" : "Target linked");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openPromptSource(prompt: Prompt) {
    if (!desktop) {
      toast("Desktop runtime required for opening source files");
      return;
    }

    try {
      await promptsApi.openSource(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealPromptPath(prompt: Prompt) {
    if (!desktop) {
      toast("Desktop runtime required for revealing files");
      return;
    }

    try {
      await promptsApi.revealPath(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openSkillSource(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for opening source files");
      return;
    }

    try {
      await skillsApi.openSource(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealSkillPath(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for revealing files");
      return;
    }

    try {
      await skillsApi.revealPath(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function submitAddProject() {
    const path = addPath.trim();
    if (!path) {
      toast("Project path is required");
      return;
    }

    try {
      const project = await recordProject.mutateAsync(path);
      setAddOpen(false);
      setAddPath("");
      setDetailId(project.id);
      setScreen("detail");
      toast(`Project recorded · key "${project.key}"`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function submitGitBaseFolder() {
    const path = baseFolderPath.trim();
    if (!path) {
      toast("Git base folder path is required");
      return;
    }

    try {
      await recordBaseFolder.mutateAsync(path);
      setBaseFolderPath("");
      setHasScanned(false);
      setScanSel({});
      scanBaseFolders.reset();
      toast(`Added base folder · ${path}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function removeGitBaseFolder(id: string, path: string) {
    try {
      await removeBaseFolder.mutateAsync(id);
      setHasScanned(false);
      setScanSel({});
      scanBaseFolders.reset();
      toast(`Removed base folder · ${path}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function submitScanGitBaseFolders() {
    if (baseFolders.length === 0) {
      toast("Add a Git base folder before scanning");
      return;
    }

    try {
      const results = await scanBaseFolders.mutateAsync();
      const selected: Record<string, boolean> = {};
      results.forEach((repo) => {
        if (repo.state === "new") selected[repo.path] = true;
      });
      setHasScanned(true);
      setScanSel(selected);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function recordSelectedScanProjects() {
    const paths = scanRes
      .filter((repo) => repo.state === "new" && scanSel[repo.path])
      .map((repo) => repo.path);

    if (paths.length === 0) return;

    try {
      await recordProjects.mutateAsync(paths);
      setBaseFoldersOpen(false);
      setHasScanned(false);
      setScanSel({});
      scanBaseFolders.reset();
      toast(`Recorded ${paths.length} ${paths.length === 1 ? "project" : "projects"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const menuItem =
    "cursor-pointer rounded-[8px] px-3.5 py-[9px] text-[12.5px] font-semibold";

  return (
    <>
      {screen === "list" ? (
        <ScreenScroll>
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
                Project
              </h1>
              <p className="mt-1.5 text-[13px] text-[#9a8f80]">
                Recorded Git repositories · workspace context entry
              </p>
            </div>
            <div className="flex gap-[9px]">
              <Button
                variant="subtle"
                size="sm"
                onClick={() => void handleRefresh()}
                disabled={isRefreshing}
              >
                <RefreshCw size={14} className={cn(isRefreshing && "animate-spin")} />
                {isRefreshing ? "Refreshing..." : "Refresh"}
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setBaseFoldersOpen(true);
                  setHasScanned(false);
                  setScanSel({});
                  scanBaseFolders.reset();
                }}
              >
                Git Base Folders
              </Button>
              <Button variant="primary" onClick={() => setAddOpen(true)}>
                Add Project
              </Button>
            </div>
          </div>

          <Card className="mt-5 overflow-hidden">
            <div
              className="grid gap-4 border-b border-nexus-panel bg-nexus-sand px-5 py-3 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
              style={{ gridTemplateColumns: LIST_COLS }}
            >
              <div />
              <div>Project</div>
              <div>Skill</div>
              <div>Prompt</div>
              <div>Session</div>
              <div />
            </div>

            {projectsQuery.isLoading ? (
              <div className="px-5 py-8 text-center text-[12.5px] text-[#b3a999]">
                Loading projects...
              </div>
            ) : null}

            {projectError ? (
              <div className="mx-5 my-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] px-4 py-3 text-[12.5px] text-nexus-crit">
                {projectError}
              </div>
            ) : null}

            {!projectsQuery.isLoading && !projectError && ordered.length === 0 ? (
              <div className="mx-5 my-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                No projects recorded yet. Add an existing Git repository root to create the first Project.
              </div>
            ) : null}

            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={(event) => void handleDragEnd(event)}
            >
              <SortableContext
                items={visibleProjects.map((p) => p.id)}
                strategy={verticalListSortingStrategy}
              >
                {visibleProjects.map((p) => {
                  const isStale = p.status === "stale";
                  return (
                    <SortableProjectRow
                      key={p.id}
                      id={p.id}
                      stale={isStale}
                      onClick={isStale ? undefined : () => { setDetailId(p.id); setScreen("detail"); }}
                    >
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <span className={cn("text-[14px] font-bold", isStale ? "text-[#6a6055]" : "text-nexus-ink")}>{p.name}</span>
                          {isStale ? (
                            <span className="rounded-[5px] bg-[#f7eccb] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#9a6f0a]">
                              Stale
                            </span>
                          ) : (
                            <span className="rounded-[5px] bg-[#e9eed8] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#5f7a3e]">
                              Active
                            </span>
                          )}
                        </div>
                        <div className={cn(
                          "mt-[3px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px]",
                          isStale ? "text-[#bca37a] line-through" : "text-[#8a8073]",
                        )}>
                          {isStale ? "Repo path no longer resolves" : p.path}
                        </div>
                      </div>
                      {isStale ? (
                        <>
                          <div />
                          <div />
                          <div />
                        </>
                      ) : (
                        <>
                          <AssetCell n={p.skills} lines={p.customSkillsDirs ?? []} />
                          <AssetCell n={p.prompts} lines={p.extraPromptFiles ?? []} />
                          <AssetCell
                            n={p.sessions}
                            lines={
                              p.sessionsDir && p.sessionsDir !== DEFAULT_SESSIONS_DIR
                                ? [p.sessionsDir]
                                : []
                            }
                          />
                        </>
                      )}
                      <div
                        onClick={(e) => openMenu(e, p.id)}
                        className="flex h-[30px] w-[30px] cursor-pointer items-center justify-center rounded-[8px] text-[16px] tracking-[2px] text-[#a99a89] hover:bg-nexus-panel hover:text-[#7a6f60]"
                      >
                        ⋯
                      </div>
                    </SortableProjectRow>
                  );
                })}
              </SortableContext>
            </DndContext>
          </Card>

          {hiddenP.length > 0 ? (
            <div className="mt-[18px] flex flex-wrap items-center gap-3 rounded-[14px] border border-dashed border-[#ddccb6] bg-nexus-panel px-[18px] py-3.5">
              <span className="text-[12px] font-bold text-[#7a6f60]">Hidden projects</span>
              {hiddenP.map((p) => (
                <div
                  key={p.id}
                  onClick={() => { setHiddenIds((ids) => ids.filter((x) => x !== p.id)); toast(`${p.name} unhidden`); }}
                  className="inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-nexus-border2 bg-nexus-card px-3 py-[5px] text-[12px] text-[#6a6055] hover:bg-nexus-sand"
                >
                  {p.name} <span className="text-[#a99a89]">· unhide</span>
                </div>
              ))}
            </div>
          ) : null}

          <p className="mt-3.5 text-[11.5px] text-[#b3a999]">
            Project identity is the folder name — used as a stable key for cross-device merge.
            Status set is <b className="text-[#9a8f80]">active / stale / hidden</b>.
          </p>
        </ScreenScroll>
      ) : dp ? (
        <ScreenScroll>
          <button
            onClick={() => setScreen("list")}
            className="mb-3.5 inline-flex items-center gap-1.5 text-[12px] text-[#9a8f80] hover:text-nexus-accent"
          >
            ← Project
          </button>

          <Card className="p-[22px]">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="min-w-0">
                <h1 className="m-0 text-[21px] font-extrabold tracking-[-.02em] text-nexus-ink">
                  {dp.name}
                </h1>
                <div className="mt-2.5 flex flex-wrap gap-x-6 gap-y-2 text-[12px]">
                  <div>
                    <span className="text-[#b3a999]">Repo&nbsp;&nbsp;</span>
                    <span className="font-mono text-[#6a6055]">{dp.path}</span>
                  </div>
                  <div>
                    <span className="text-[#b3a999]">Key&nbsp;&nbsp;</span>
                    <span className="font-mono text-[#6a6055]">{dp.key}</span>
                    <span className="ml-1.5 text-[10px] text-[#bca37a]">folder name</span>
                  </div>
                  <div>
                    <span className="text-[#b3a999]">Session dir&nbsp;&nbsp;</span>
                    <span className="font-mono text-[#6a6055]">{dp.sessionsDir}</span>
                  </div>
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button variant="primary" size="sm" className="px-3.5" onClick={() => toast(`Archive now → ${dp.name} (one-way Backup)`)}>
                  Archive now
                </Button>
                <Button variant="secondary" size="sm" className="px-3.5" onClick={() => toast(`Pull now → ${dp.name} (one-way Restore/Pull)`)}>
                  Pull now
                </Button>
                <Button variant="secondary" size="sm" className="px-3.5" onClick={() => go("sync")}>
                  Open in Sync
                </Button>
              </div>
            </div>
          </Card>

          {/* Skill table */}
          <Card className="mt-4 overflow-hidden">
            <div className="flex items-center justify-between gap-2.5 px-5 pb-1 pt-4">
              <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                Skill
              </span>
              <div className="flex items-center gap-3">
                <button
                  onClick={() => setCustomDirsOpen(true)}
                  className="text-[11px] font-semibold text-nexus-accent hover:underline"
                >
                  Custom skills dirs
                  {dp.customSkillsDirs && dp.customSkillsDirs.length > 0
                    ? ` · ${dp.customSkillsDirs.length}`
                    : ""}
                </button>
                <span className="text-[11px] text-[#b3a999]">
                  project scope · {dpSkills.length} {dpSkills.length === 1 ? "skill" : "skills"}
                </span>
              </div>
            </div>
            <div
              className="grid items-center gap-4 px-5 py-2.5 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
              style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
            >
              <div>Skill</div>
              <div className="text-center">Distribution</div>
              <div className="text-center">Disable invoke</div>
              <div className="text-right">Source file</div>
            </div>
            {skillsQuery.isLoading && dpSkills.length === 0 ? (
              <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                Loading project skills...
              </div>
            ) : skillError ? (
              <div className="mx-5 mb-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
                {skillError}
              </div>
            ) : dpSkills.length > 0 ? (
              dpSkills.map((k) => (
                <SkillRow
                  key={k.id}
                  skill={k}
                  mode="project"
                  projectName={dp.name}
                  onToggleCell={(a) => void toggleCell(k, a)}
                  onToggleDmi={() => void toggleDmi(k)}
                  onPropagateGlobal={(entry) => void propagateGlobal(k, entry)}
                  onUnpropagateGlobal={() => void unpropagateGlobal(k)}
                  onOpen={() => void openSkillSource(k)}
                  onReveal={() => void revealSkillPath(k)}
                />
              ))
            ) : (
              <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                No project skills recorded for this repository.
              </div>
            )}
            <button onClick={() => go("skill")} className="mx-5 mb-[18px] inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
              Open in Skill →
            </button>
          </Card>

          {/* Prompt table */}
          <Card className="mt-4 overflow-hidden">
            <div className="flex items-center justify-between gap-2.5 px-5 pb-1 pt-4">
              <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                Prompt
              </span>
              <div className="flex items-center gap-3">
                <button
                  onClick={() => setExtraFilesOpen(true)}
                  className="text-[11px] font-semibold text-nexus-accent hover:underline"
                >
                  Custom prompt files
                  {dp.extraPromptFiles && dp.extraPromptFiles.length > 0
                    ? ` · ${dp.extraPromptFiles.length}`
                    : ""}
                </button>
                <span className="text-[11px] text-[#b3a999]">
                  project scope · {dpPrompts.length} {dpPrompts.length === 1 ? "prompt" : "prompts"}
                </span>
              </div>
            </div>
            <div
              className="grid items-center gap-4 px-5 py-2.5 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
              style={{ gridTemplateColumns: "1fr 196px 132px" }}
            >
              <div>Prompt</div>
              <div className="text-center">Distribution</div>
              <div className="text-right">Source file</div>
            </div>
            {promptsQuery.isLoading && dpPrompts.length === 0 ? (
              <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                Loading project prompts...
              </div>
            ) : promptError ? (
              <div className="mx-5 mb-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
                {promptError}
              </div>
            ) : dpPrompts.length > 0 ? (
              dpPrompts.map((p) => (
                <div
                  key={p.id}
                  className="grid items-center gap-4 border-t border-[#f3eee5] px-5 py-[14px]"
                  style={{ gridTemplateColumns: "1fr 196px 132px" }}
                >
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[14px] font-bold text-nexus-ink">{p.name}</span>
                      <SourceBadge agent={srcAgentOf(p.cells)} />
                    </div>
                    <div className="mt-[3px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#a99a89]">
                      {p.path}
                    </div>
                  </div>
                  <AgentMatrixCells
                    cells={p.cells}
                    agents={PROJECT_PROMPT_AGENTS}
                    onToggle={(a) => void togglePromptCell(p, a)}
                  />
                  <div className="flex flex-col items-end gap-[5px]">
                    <span
                      onClick={() => void openPromptSource(p)}
                      className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-nexus-accent hover:underline"
                    >
                      Open source
                    </span>
                    <span
                      onClick={() => void revealPromptPath(p)}
                      className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-[#a99a89] hover:underline"
                    >
                      Reveal path
                    </span>
                  </div>
                </div>
              ))
            ) : (
              <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                No project prompts discovered. Add AGENTS.md / CLAUDE.md at the repo root, or register extra prompt files.
              </div>
            )}
            <div className="mx-5 mb-[18px] flex items-center justify-between gap-3">
              <button onClick={() => go("prompt")} className="inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
                Open in Prompt →
              </button>
              <MatrixLegend />
            </div>
          </Card>

          {/* Session panel */}
          <Card className="mt-4 p-5">
            <div className="mb-3.5 flex items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Session
                </span>
                <button
                  onClick={() => setSessionDirOpen(true)}
                  className="text-[11px] font-semibold text-nexus-accent hover:underline"
                >
                  Configure session dir
                  {dp.sessionsDir && dp.sessionsDir !== DEFAULT_SESSIONS_DIR
                    ? ` · ${dp.sessionsDir}`
                    : ""}
                </button>
              </div>
              <Segmented<DetailSource>
                options={[
                  { value: "local", label: "Local" },
                  { value: "cloud", label: "Cloud" },
                ]}
                value={detailSource}
                onChange={setDetailSource}
                size="md"
              />
            </div>
            {detailSessionsQuery.isLoading && dpSessions.length === 0 ? (
              <div className="rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                Loading {detailSource} sessions...
              </div>
            ) : detailSessionError ? (
              <div className="rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
                {detailSessionError}
              </div>
            ) : dpSessions.length > 0 ? (
              <div className="flex flex-col gap-0.5">
                {dpSessions.map((se) => (
                  <div
                    key={se.id}
                    className="flex items-center justify-between gap-3 rounded-[10px] px-[11px] py-2.5 hover:bg-nexus-sand"
                  >
                    <div className="min-w-0">
                      <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12.5px] font-bold text-nexus-body">
                        {se.title}
                      </div>
                      <div className="overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-[#b3a999]">
                        {se.excerpt}
                      </div>
                    </div>
                    <div className="flex-none whitespace-nowrap text-[11px] text-[#c3b9a8]">
                      {se.updated}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                {detailSource === "cloud"
                  ? "No archived sessions in the Cloud for this project yet."
                  : "No local sessions in the session directory."}
              </div>
            )}
            <button onClick={() => go("session")} className="mt-3 inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
              Open in Session →
            </button>
          </Card>

          {/* Sync summary */}
          <Card className="mt-4 p-5">
            <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
              Sync summary
            </span>
            <div className="mt-3.5 flex flex-col gap-0.5">
              {[
                {
                  label: "Skill Distribution",
                  detail: `${dpSkills.length} project ${dpSkills.length === 1 ? "skill" : "skills"} recorded`,
                  status: dpSkills.length > 0 ? "Recorded" : "None",
                  fg: dpSkills.length > 0 ? "#5f7a3e" : "#a99a89",
                  dot: dpSkills.length > 0 ? palette.good : "#d9c9b3",
                },
                {
                  label: "Session Backup",
                  detail: sessionBackup?.task.target ?? "No Session Backup task recorded",
                  status: sessionBackupSummary.label,
                  fg: sessionBackupSummary.fg,
                  dot: sessionBackupSummary.dot,
                },
                {
                  label: "Custom Task Groups",
                  detail: `${dp.sync} project-bound sync ${dp.sync === 1 ? "record" : "records"}`,
                  status: dp.sync > 0 ? "Recorded" : "None",
                  fg: dp.sync > 0 ? "#5f7a3e" : "#a99a89",
                  dot: dp.sync > 0 ? palette.good : "#d9c9b3",
                },
              ].map((sy) => (
                <div
                  key={sy.label}
                  className="grid items-center gap-3.5 rounded-[10px] p-[11px] hover:bg-nexus-sand"
                  style={{ gridTemplateColumns: "180px 1fr 120px" }}
                >
                  <div className="text-[12.5px] font-bold text-nexus-body">{sy.label}</div>
                  <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#a99a89]">
                    {sy.detail}
                  </div>
                  <div
                    className="inline-flex items-center gap-1.5 justify-self-end text-[11.5px] font-bold"
                    style={{ color: sy.fg }}
                  >
                    <Dot color={sy.dot} /> {sy.status}
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </ScreenScroll>
      ) : (
        <ScreenScroll>
          <button
            onClick={() => setScreen("list")}
            className="mb-3.5 inline-flex items-center gap-1.5 text-[12px] text-[#9a8f80] hover:text-nexus-accent"
          >
            ← Project
          </button>
          <Card className="p-[22px] text-[12.5px] text-[#9a8f80]">
            No project is selected.
          </Card>
        </ScreenScroll>
      )}

      {/* Add Project modal */}
      <Modal open={addOpen} onClose={() => setAddOpen(false)} className="w-[480px]">
        <ModalHeader title="Add Project" subtitle="Record a single Git repository root" />
        <div className="flex flex-col gap-4 px-[22px] py-5">
          <div>
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Repository root</div>
            <input
              value={addPath}
              onChange={(event) => setAddPath(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !recordProject.isPending) {
                  void submitAddProject();
                }
              }}
              placeholder="/Users/lumiarna/Workspace/agent-nexus"
              className="w-full rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055] outline-none focus:border-nexus-accent"
              autoFocus
            />
          </div>
          <div>
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Project Key</div>
            <div className="flex items-center gap-2 rounded-[10px] border border-nexus-border2 bg-nexus-bg px-3 py-[9px] font-mono text-[12.5px] text-[#6a6055]">
              {addKey || "folder-name"}
            </div>
            <div className="mt-2 rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-[11px] text-[11.5px] leading-[1.55] text-[#8a7a68]">
              The Project Key is always the folder name. It is the stable identity used to merge the
              same project across devices — there is no manual key in the MVP.
            </div>
          </div>
        </div>
        <ModalFooter>
          <Button variant="subtle" onClick={() => setAddOpen(false)}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={() => void submitAddProject()}
            disabled={recordProject.isPending || !addPath.trim()}
          >
            {recordProject.isPending ? "Recording..." : "Record project"}
          </Button>
        </ModalFooter>
      </Modal>

      {/* Custom skills dirs modal */}
      {dp ? (
        <StringListConfigModal
          open={customDirsOpen}
          onClose={() => setCustomDirsOpen(false)}
          title="Project custom skills dirs"
          subtitle="Extra scan sources alongside the fixed Agent project skills dirs"
          items={dp.customSkillsDirs ?? []}
          onAdd={(dirs) => setCustomSkillsDirs.mutateAsync({ projectId: dp.id, dirs })}
          onRemove={(dirs) => setCustomSkillsDirs.mutateAsync({ projectId: dp.id, dirs })}
          placeholder="skills  ·  .nexus/skills  ·  /abs/path/to/skills"
          addLabel="Add dir"
          initialInput="skills"
          busy={setCustomSkillsDirs.isPending}
          messages={{
            added: (dir) => `Added custom skills dir · ${dir}`,
            removed: (dir) => `Removed custom skills dir · ${dir}`,
            duplicate: "Directory already added",
          }}
          emptyHint="No custom skills dirs. Relative paths resolve against the Project root."
          renderBadge={(dir) => {
            const external = /^(~|\/|[A-Za-z]:[\\/])/.test(dir);
            return (
              <span
                className="flex-none rounded-[5px] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em]"
                style={{
                  color: external ? "#9a6f0a" : "#5f7a3e",
                  background: external ? "#f7eccb" : "#e9eed8",
                }}
              >
                {external ? "External" : "In repo"}
              </span>
            );
          }}
          help={
            <>
              Each dir is scanned for real Skill folders (containing{" "}
              <span className="font-mono">SKILL.md</span>) as Project custom sources — they show no
              Agent Matrix and can be propagated to Global. A dir that resolves to a fixed Agent
              project skills dir is rejected. Removing a dir drops its custom Skills on the next
              scan; managed Global placements fall back to none.
            </>
          }
        />
      ) : null}

      {/* Extra prompt files modal */}
      {dp ? (
        <StringListConfigModal
          open={extraFilesOpen}
          onClose={() => setExtraFilesOpen(false)}
          title="Project extra prompt files"
          subtitle="Extra Prompt files scanned alongside the primary AGENTS.md / CLAUDE.md"
          items={dp.extraPromptFiles ?? []}
          onAdd={(files) => setExtraPromptFiles.mutateAsync({ projectId: dp.id, files })}
          onRemove={(files) => setExtraPromptFiles.mutateAsync({ projectId: dp.id, files })}
          validate={(file) =>
            matchesPromptGlob(file) ? null : "File must match AGENTS*.md or CLAUDE*.md"
          }
          placeholder="AGENTS.local.md  ·  docs/CLAUDE.md"
          addLabel="Add file"
          busy={setExtraPromptFiles.isPending}
          messages={{
            added: (file) => `Added extra prompt file · ${file}`,
            removed: (file) => `Removed extra prompt file · ${file}`,
            duplicate: "File already added",
          }}
          emptyHint="No extra prompt files. Paths resolve against the Project root."
          renderBadge={(file) => {
            const owner = PROMPT_FILE_GLOBS.find((g) =>
              g.re.test(file.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? ""),
            )?.agent;
            return owner ? (
              <span className="flex-none rounded-[5px] bg-[#e9eed8] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#5f7a3e]">
                {owner}
              </span>
            ) : null;
          }}
          help={
            <>
              Each file&apos;s name must match an Agent prompt-file glob —{" "}
              <span className="font-mono">AGENTS*.md</span> (Generic Agent) or{" "}
              <span className="font-mono">CLAUDE*.md</span> (Claude Code). The matching Agent
              becomes the Source Agent; files that match neither are rejected. This widens the
              Prompt scan inside an existing Agent namespace — it does not create a new source.
            </>
          }
        />
      ) : null}

      {/* Session dir modal */}
      {dp ? (
        <SingleValueConfigModal
          open={sessionDirOpen}
          onClose={() => setSessionDirOpen(false)}
          title="Configure session dir"
          subtitle="Override the single Session Directory for this Project"
          label="Session directory"
          initialValue={
            dp.sessionsDir && dp.sessionsDir !== DEFAULT_SESSIONS_DIR ? dp.sessionsDir : ""
          }
          placeholder={DEFAULT_SESSIONS_DIR}
          onSubmit={async (dir) => {
            const project = await setSessionsDir.mutateAsync({ projectId: dp.id, dir });
            return project.sessionsDir;
          }}
          busy={setSessionsDir.isPending}
          messages={{
            set: (canonical) => `Session dir set · ${canonical}`,
            cleared: "Session dir restored to default",
          }}
          help={
            <>
              A Project always has exactly one Session Directory — this is a deliberate constraint,
              not an MVP limit. Relative paths resolve against the Project root. Leave empty to
              restore the default <span className="font-mono">{DEFAULT_SESSIONS_DIR}</span>.
            </>
          }
        />
      ) : null}

      {/* Git Base Folders modal */}
      <Modal open={baseFoldersOpen} onClose={() => setBaseFoldersOpen(false)} className="max-h-[88vh] w-[560px]">
        <ModalHeader
          title="Git Base Folders"
          subtitle="Manage discovery directories and scan for Git repositories"
        />
        <div className="flex flex-col gap-4 px-[22px] py-5">
          {/* Registered folders */}
          <div>
            <div className="mb-2 flex items-center gap-2">
              <div className="text-[12px] font-bold text-[#6a6055]">Registered folders</div>
            </div>
            <div className="mb-3 flex gap-2">
              <input
                value={baseFolderPath}
                onChange={(event) => setBaseFolderPath(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !recordBaseFolder.isPending) {
                    void submitGitBaseFolder();
                  }
                }}
                placeholder="/Users/lumiarna/Workspace"
                className="min-w-0 flex-1 rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055] outline-none focus:border-nexus-accent"
              />
              <Button
                variant="secondary"
                size="sm"
                className="rounded-[10px]"
                onClick={() => void submitGitBaseFolder()}
                disabled={recordBaseFolder.isPending || !baseFolderPath.trim()}
              >
                {recordBaseFolder.isPending ? "Adding..." : "Add folder"}
              </Button>
            </div>
            <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
              {baseFoldersQuery.isLoading ? (
                <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                  Loading base folders...
                </div>
              ) : null}

              {baseFoldersError ? (
                <div className="px-3.5 py-[11px] text-[12px] text-nexus-crit">
                  {baseFoldersError}
                </div>
              ) : null}

              {!baseFoldersQuery.isLoading && !baseFoldersError && baseFolders.length === 0 ? (
                <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                  No base folders registered.
                </div>
              ) : null}

              {baseFolders.map((bf, i) => (
                <div
                  key={bf.id}
                  className={cn(
                    "flex items-center justify-between gap-3 px-3.5 py-[11px]",
                    (i > 0 || baseFoldersQuery.isLoading || baseFoldersError) &&
                      "border-t border-[#f3eee5]",
                  )}
                >
                  <div className="min-w-0">
                    <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-nexus-body">
                      {bf.path}
                    </div>
                    <div className="mt-0.5 text-[10.5px] text-[#b3a999]">Added {bf.addedAt}</div>
                  </div>
                  <button
                    onClick={() => void removeGitBaseFolder(bf.id, bf.path)}
                    disabled={removeBaseFolder.isPending}
                    className="flex-none text-[11px] font-semibold text-nexus-crit hover:underline disabled:cursor-wait disabled:opacity-60"
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
            <div className="mt-1.5 text-[11px] text-[#b3a999]">
              A base folder is only a discovery input — removing it does not delete any recorded Projects.
            </div>
          </div>

          {/* Scan section */}
          <div className="border-t border-[#f3eee5] pt-4">
            <div className="flex items-center gap-2">
              <div className="text-[12px] font-bold text-[#6a6055]">Scan &amp; discover</div>
              <Button
                variant="primary"
                size="sm"
                className="ml-auto rounded-[10px]"
                onClick={() => void submitScanGitBaseFolders()}
                disabled={scanBaseFolders.isPending || baseFolders.length === 0}
              >
                {scanBaseFolders.isPending ? "Scanning..." : "Scan all folders"}
              </Button>
            </div>
            {scanBaseFolders.error ? (
              <div className="mt-2 text-[11.5px] text-nexus-crit">
                {getErrorMessage(scanBaseFolders.error)}
              </div>
            ) : null}
          </div>

          {hasScanned ? (
            <div>
              <div className="mb-2 text-[12px] font-bold text-[#6a6055]">
                Discovered repositories{" "}
                <span className="font-medium text-[#b3a999]">
                  {"\u00b7"} {scanRes.length} found {"\u00b7"} {newCount} new {"\u00b7"}{" "}
                  {scanRes.length - newCount} already recorded
                </span>
              </div>
              <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
                {scanRes.length === 0 ? (
                  <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                    No Git repositories found in registered base folders.
                  </div>
                ) : null}

                {scanRes.map((r, i) => {
                  const isNew = r.state === "new";
                  const on = isNew && !!scanSel[r.path];
                  return (
                    <div
                      key={r.path}
                      onClick={() => isNew && setScanSel((s) => ({ ...s, [r.path]: !s[r.path] }))}
                      className={cn(
                        "flex items-center gap-[11px] px-3.5 py-[11px]",
                        i > 0 && "border-t border-[#f3eee5]",
                        isNew ? "cursor-pointer" : "cursor-default opacity-70",
                      )}
                      style={{ background: on ? "#fbf6ef" : "#fcf9f4" }}
                    >
                      <span
                        className="inline-flex h-[18px] w-[18px] flex-none items-center justify-center rounded-[5px] text-[11px] font-extrabold text-white"
                        style={{
                          background: on ? "#9d7a64" : "#fff",
                          border: `1px solid ${on ? "#9d7a64" : isNew ? "#d9c4b8" : "#e7ddce"}`,
                        }}
                      >
                        {on ? "✓" : ""}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-nexus-body">
                          {r.path}
                        </div>
                        <div className="text-[10.5px] text-[#b3a999]">
                          key <span className="font-mono text-[#9a8f80]">{r.key}</span>
                        </div>
                      </div>
                      <span
                        className="flex-none rounded-[5px] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em]"
                        style={{
                          color: isNew ? "#5f7a3e" : "#a99a89",
                          background: isNew ? "#e9eed8" : "#f0e8db",
                        }}
                      >
                        {isNew ? "New" : "Recorded"}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : null}
        </div>
        <ModalFooter>
          <Button variant="subtle" onClick={() => setBaseFoldersOpen(false)}>
            Close
          </Button>
          {hasScanned && selCount > 0 && (
            <Button
              variant="primary"
              onClick={() => void recordSelectedScanProjects()}
              disabled={recordProjects.isPending}
            >
              {recordProjects.isPending
                ? "Recording..."
                : `Record ${selCount} ${selCount === 1 ? "project" : "projects"}`}
            </Button>
          )}
        </ModalFooter>
      </Modal>

      {/* Delete confirm modal */}
      <Modal
        open={!!del}
        onClose={() => { setDeleteId(null); setDeleteAck(false); }}
        overlayClassName="bg-[rgba(42,33,28,.40)]"
      >
        {del ? (
          <>
            <ModalHeader
              title={`Delete ${del.name} permanently?`}
              titleClassName="text-nexus-crit"
              subtitle="This cannot be undone. The following associated data will be removed:"
            />
            <div className="px-[22px] py-[18px]">
              <div className="flex flex-col gap-0.5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-2">
                {[
                  { label: "Archived & local sessions", value: `${del.sessions} files` },
                  { label: "Project skills", value: `${del.skills}` },
                  { label: "Sync tasks", value: `${del.sync}` },
                  { label: "Session backup record", value: "1" },
                ].map((d) => (
                  <div key={d.label} className="flex items-center justify-between gap-2.5 px-[11px] py-2">
                    <span className="text-[12.5px] text-[#6a5550]">{d.label}</span>
                    <span className="text-[12.5px] font-bold text-nexus-crit">{d.value}</span>
                  </div>
                ))}
              </div>
              <label
                onClick={() => setDeleteAck((a) => !a)}
                className="mt-3.5 flex cursor-pointer items-center gap-[9px]"
              >
                <span
                  className="inline-flex h-[18px] w-[18px] flex-none items-center justify-center rounded-[5px] text-[11px] font-extrabold text-white"
                  style={{
                    background: deleteAck ? "#b55440" : "#fff",
                    border: `1px solid ${deleteAck ? "#b55440" : "#d9c4b8"}`,
                  }}
                >
                  {deleteAck ? "✓" : ""}
                </span>
                <span className="text-[12.5px] text-[#6a6055]">
                  I understand this permanently deletes the project and its data.
                </span>
              </label>
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => { setDeleteId(null); setDeleteAck(false); }}>
                Cancel
              </Button>
              <button
                onClick={() => {
                  if (!deleteAck || deleteProject.isPending) return;
                  const id = del.id;
                  const n = del.name;
                  setDeleteId(null);
                  setDeleteAck(false);
                  deleteProject.mutateAsync(id).then(
                    () => toast(`${n} permanently deleted`),
                    (error: unknown) => toast(getErrorMessage(error)),
                  );
                }}
                className={cn(
                  "rounded-full px-[18px] py-[9px] text-[12.5px] font-bold text-white",
                  deleteAck && !deleteProject.isPending
                    ? "cursor-pointer bg-nexus-crit shadow-[0_2px_8px_rgba(181,84,64,.32)]"
                    : "cursor-not-allowed bg-[#d9b6ab]",
                )}
              >
                {deleteProject.isPending ? "Deleting..." : "Delete permanently"}
              </button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>

      {/* Overflow menu */}
      {menu ? (
        <>
          <div onClick={() => setMenu(null)} className="fixed inset-0 z-[50]" />
          <div
            className="fixed z-[51] min-w-[148px] animate-ann-fade rounded-[12px] border border-nexus-border2 bg-nexus-card p-[5px] shadow-[0_8px_24px_rgba(50,40,25,.18)]"
            style={{ top: menu.y, right: menu.right }}
          >
            {menuProject?.status === "stale" ? (
              <div
                onClick={() => { const n = menuProject.name; setMenu(null); toast(`Relocate ${n} — pick the new repository root`); }}
                className={cn(menuItem, "text-[#6a6055] hover:bg-nexus-bg")}
              >
                Relocate repo
              </div>
            ) : null}
            <div
              onClick={() => {
                const p = menuProject;
                setMenu(null);
                if (p) { setHiddenIds((ids) => [...ids, p.id]); toast(`${p.name} hidden`); }
              }}
              className={cn(menuItem, "text-[#6a6055] hover:bg-nexus-bg")}
            >
              Hide
            </div>
            <div
              onClick={() => { const id = menu.id; setMenu(null); setDeleteId(id); setDeleteAck(false); }}
              className={cn(menuItem, "text-nexus-crit hover:bg-[#f8ebe6]")}
            >
              Delete…
            </div>
          </div>
        </>
      ) : null}
    </>
  );
}
