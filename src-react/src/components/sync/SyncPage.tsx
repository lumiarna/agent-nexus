import { useEffect, useState } from "react";
import { Copy, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dot, Input, Spinner } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Chip, Segmented } from "@/components/ui/segmented";
import { ScreenScroll } from "@/components/shell/screen";
import { formatProjectSymlinkDisplayPath } from "@/components/sync/pathDisplay";
import {
  useCreateTaskGroupMutation,
  useDeleteProjectSymlinkMutation,
  useDeleteTaskMutation,
  useProjectSymlinksQuery,
  useRunTaskMutation,
  useTaskGroupsQuery,
} from "@/lib/query/sync";
import { nexus } from "@/lib/mock";
import { palette } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type {
  SystemSyncRow,
  Task,
  TaskAction,
  TaskDirection,
  TaskGroup,
  TaskStatus,
  LocationType,
} from "@/types";

const SCHEDULE_PRESETS = [
  { label: "Hourly", expr: "0 * * * *" },
  { label: "Daily 05:00", expr: "0 5 * * *" },
  { label: "Weekly Sun 03:00", expr: "0 3 * * 0" },
];
const TASK_COLS = "24px 132px 1.3fr 1.4fr 150px";
const LINK_COLS =
  "minmax(70px,.6fr) minmax(0,1.6fr) minmax(70px,.6fr) minmax(0,1.6fr) 60px";

function dirColors(d: TaskDirection): { fg: string; bg: string } {
  if (d === "Push") return { fg: "#9a6f0a", bg: "#f7eccb" };
  if (d === "Pull") return { fg: "#3f6f55", bg: "#dcebe0" };
  return { fg: "#4a6a8a", bg: "#e2ebf2" };
}

function statusOf(st: TaskStatus): { label: string; fg: string; dot: string } {
  if (st === "ok") return { label: "OK", fg: "#5f7a3e", dot: palette.good };
  if (st === "pending") return { label: "Pending", fg: "#9a6f0a", dot: palette.warn };
  if (st === "failed") return { label: "Failed", fg: palette.crit, dot: palette.crit };
  return { label: "Never", fg: "#a99a89", dot: "#d9c9b3" };
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

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    toast(`Copied · ${path}`);
  } catch (error) {
    toast(getErrorMessage(error));
  }
}

function CopyPathButton({ path }: { path: string }) {
  return (
    <button
      type="button"
      title="Copy absolute path"
      onClick={() => void copyPath(path)}
      className="flex-none text-[#c3b9a8] transition-colors hover:text-nexus-accent"
    >
      <Copy size={12} />
    </button>
  );
}

function cronHuman(expr: string): string {
  const m: Record<string, string> = {
    "0 * * * *": "Every hour, on the hour.",
    "0 5 * * *": "Every day at 05:00.",
    "0 3 * * 0": "Every Sunday at 03:00.",
    "0 4 * * *": "Every day at 04:00.",
  };
  return m[expr] ?? "Custom schedule expression.";
}

function DirBadge({ direction }: { direction: TaskDirection }) {
  const c = dirColors(direction);
  return (
    <span
      className="inline-flex w-fit items-center justify-center rounded-[5px] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.03em]"
      style={{ color: c.fg, background: c.bg }}
    >
      {direction}
    </span>
  );
}

let ntSeq = 0;
function newTask(): FormTask {
  return {
    id: `nt${ntSeq++}_${Math.random().toString(36).slice(2, 6)}`,
    action: "Symlink",
    sourceType: "Local",
    source: "",
    targets: [{ type: "Local", path: "" }],
    schedule: "manual",
  };
}

interface FormTarget {
  type: LocationType;
  path: string;
}
interface FormTask {
  id: string;
  action: TaskAction;
  sourceType: LocationType;
  source: string;
  targets: FormTarget[];
  schedule: string;
}
interface FormState {
  name: string;
  tasks: FormTask[];
}
interface SchedState {
  groupId: string;
  taskId: string;
  taskName: string;
  mode: "manual" | "cron";
  cronExpr: string;
}

export function SyncPage() {
  const taskGroupsQuery = useTaskGroupsQuery();
  const createTaskGroupMutation = useCreateTaskGroupMutation();
  const deleteTaskMutation = useDeleteTaskMutation();
  const runTaskMutation = useRunTaskMutation();
  const projectSymlinksQuery = useProjectSymlinksQuery();
  const deleteProjectSymlinkMutation = useDeleteProjectSymlinkMutation();
  const [groups, setGroups] = useState<TaskGroup[]>(() => nexus.taskGroups());
  const [templates] = useState(() => nexus.templates());
  const [system] = useState(() => nexus.systemSync());
  const [openSec, setOpenSec] = useState({ skill: false, prompt: false, backup: false });
  const [createOpen, setCreateOpen] = useState(false);
  const [tpl, setTpl] = useState("blank");
  const [form, setForm] = useState<FormState>({ name: "", tasks: [newTask()] });
  const [sched, setSched] = useState<SchedState | null>(null);
  const [dragGroupId, setDragGroupId] = useState<string | null>(null);
  const [dragTask, setDragTask] = useState<{ groupId: string; taskId: string } | null>(null);
  const projectSymlinks = projectSymlinksQuery.data ?? [];
  const projectSymlinkError = projectSymlinksQuery.error
    ? getErrorMessage(projectSymlinksQuery.error)
    : null;

  useEffect(() => {
    if (taskGroupsQuery.data) {
      setGroups(taskGroupsQuery.data);
    }
  }, [taskGroupsQuery.data]);

  // ── mutations ──
  function updateTask(groupId: string, taskId: string, patch: Partial<Task>) {
    setGroups((gs) =>
      gs.map((g) =>
        g.id !== groupId
          ? g
          : { ...g, tasks: g.tasks.map((t) => (t.id !== taskId ? t : { ...t, ...patch })) },
      ),
    );
  }

  function reorderGroups(fromId: string, toId: string) {
    if (!fromId || fromId === toId) return;
    setGroups((gs) => {
      const a = [...gs];
      const fi = a.findIndex((g) => g.id === fromId);
      const ti = a.findIndex((g) => g.id === toId);
      if (fi < 0 || ti < 0) return gs;
      const [m] = a.splice(fi, 1);
      a.splice(ti, 0, m);
      return a;
    });
  }
  function reorderTask(groupId: string, fromId: string, toId: string) {
    if (!fromId || fromId === toId) return;
    setGroups((gs) =>
      gs.map((g) => {
        if (g.id !== groupId) return g;
        const a = [...g.tasks];
        const fi = a.findIndex((t) => t.id === fromId);
        const ti = a.findIndex((t) => t.id === toId);
        if (fi < 0 || ti < 0) return g;
        const [m] = a.splice(fi, 1);
        a.splice(ti, 0, m);
        return { ...g, tasks: a };
      }),
    );
  }

  async function deleteTask(groupId: string, task: Task) {
    try {
      await deleteTaskMutation.mutateAsync(task.id);
      setGroups((gs) =>
        gs.map((g) =>
          g.id === groupId ? { ...g, tasks: g.tasks.filter((t) => t.id !== task.id) } : g,
        ),
      );
      toast(`Deleted · ${task.direction} · ${task.source || "task"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function runTask(groupId: string, task: Task) {
    try {
      const updated = await runTaskMutation.mutateAsync(task.id);
      updateTask(groupId, task.id, updated);
      toast(`Run complete · ${task.direction} · ${task.source || "task"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function runGroup(group: TaskGroup) {
    const runnable = group.tasks.filter((task) => task.action === "Copy");
    if (!runnable.length) return;

    try {
      for (const task of runnable) {
        const updated = await runTaskMutation.mutateAsync(task.id);
        updateTask(group.id, task.id, updated);
      }
      toast(`Run group complete · ${group.name}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function deleteProjectSymlink(targetPath: string) {
    try {
      await deleteProjectSymlinkMutation.mutateAsync(targetPath);
      toast("Project symlink deleted");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  // ── create-modal helpers ──
  function openCreate() {
    setTpl("blank");
    setForm({ name: "", tasks: [newTask()] });
    setCreateOpen(true);
  }
  function patchFormTask(i: number, p: Partial<FormTask>) {
    setForm((f) => ({ ...f, tasks: f.tasks.map((t, j) => (j === i ? { ...t, ...p } : t)) }));
  }
  function pickTemplate(id: string) {
    const t = templates.find((x) => x.id === id);
    setTpl(id);
    setForm({
      name: id === "blank" || !t ? "" : t.name,
      tasks:
        t && t.tasks.length
          ? t.tasks.map((tk) => ({ ...newTask(), action: tk.action, sourceType: tk.sourceType, source: tk.source, targets: [{ type: tk.targetType, path: tk.target }], schedule: tk.action === "Symlink" ? "manual" : tk.schedule }))
          : [newTask()],
    });
  }
  async function submitCreate() {
    const name = form.name.trim() || "Untitled group";
    const tasks = form.tasks.flatMap((tk) => {
      const validTargets = tk.targets.filter((t) => t.path.trim());
      if (!validTargets.length) validTargets.push({ type: tk.targets[0]?.type ?? "Local", path: "(target)" });
      return validTargets.map((tgt) => ({
          action: tk.action,
          sourceType: tk.sourceType,
          source: tk.source || "(source)",
          targetType: tgt.type,
          target: tgt.path.trim() || "(target)",
          schedule: (tk.schedule || "manual").trim() || "manual",
      }));
    });
    try {
      const created = await createTaskGroupMutation.mutateAsync({ name, tasks });
      setGroups((gs) => [...gs.filter((g) => g.id !== created.id), created]);
      setCreateOpen(false);
      toast(`Task group "${name}" created · ${tasks.length} ${tasks.length === 1 ? "task" : "tasks"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const sysSections: {
    key: keyof typeof openSec;
    title: string;
    managedBy: string;
    count: string;
    rows: SystemSyncRow[];
  }[] = [
    { key: "skill", title: "Skill Distribution", managedBy: "Managed by Skill", count: `${system.skill.length} records`, rows: system.skill },
    { key: "prompt", title: "Prompt Distribution", managedBy: "Managed by Prompt", count: `${system.prompt.length} records`, rows: system.prompt },
    { key: "backup", title: "Session Backup", managedBy: "Managed by Session", count: `${system.backup.length} projects`, rows: system.backup },
  ];

  return (
    <>
      <ScreenScroll>
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">Sync</h1>
            <p className="mt-1.5 text-[13px] text-[#9a8f80]">
              Task workbench · groups organize tasks · each task carries its own direction, action
              &amp; schedule
            </p>
          </div>
          <Button variant="primary" onClick={openCreate}>
            + Create custom task
          </Button>
        </div>

        <div className="mt-[22px] flex items-center gap-2.5">
          <h2 className="m-0 whitespace-nowrap text-[15px] font-extrabold text-nexus-ink">
            Your Task Groups
          </h2>
          <span className="text-[11px] text-[#b3a999]">
            Drag groups, or tasks within a group, to reorder · serial execution top-to-bottom
          </span>
        </div>

        <div className="mt-3.5 flex flex-col gap-3.5">
          {taskGroupsQuery.isLoading ? (
            <div className="flex items-center gap-2 rounded-[18px] border border-nexus-border bg-nexus-card px-6 py-10 text-[12.5px] text-[#9a8f80] shadow-[0_1px_14px_rgba(50,40,25,.05)]">
              <Spinner /> Loading task groups...
            </div>
          ) : null}
          {!taskGroupsQuery.isLoading && groups.map((g) => {
            const gDragging = dragGroupId === g.id;
            return (
              <div
                key={g.id}
                draggable
                onDragStart={(e) => {
                  setDragGroupId(g.id);
                  e.dataTransfer.effectAllowed = "move";
                  try { e.dataTransfer.setData("text/plain", g.id); } catch { /* noop */ }
                }}
                onDragOver={(e) => { if (dragGroupId) e.preventDefault(); }}
                onDrop={(e) => { if (dragGroupId) { e.preventDefault(); reorderGroups(dragGroupId, g.id); setDragGroupId(null); } }}
                onDragEnd={() => setDragGroupId(null)}
                className={cn(
                  "overflow-hidden rounded-[18px] border bg-nexus-card transition-[box-shadow,opacity]",
                  gDragging
                    ? "border-nexus-accent opacity-60 shadow-[0_8px_28px_rgba(50,40,25,.16)]"
                    : "border-nexus-border shadow-[0_1px_14px_rgba(50,40,25,.05)]",
                )}
              >
                <div className="flex flex-wrap items-center gap-[11px] border-b border-[#f3eee5] px-5 py-[15px]">
                  <span title="Drag to reorder group" className="cursor-grab text-[13px] leading-none tracking-[-1px] text-[#cabfae]">
                    ⠿
                  </span>
                  <div className="text-[14.5px] font-extrabold text-nexus-ink">{g.name}</div>
                  <span className="text-[11px] text-[#b3a999]">
                    {g.tasks.length} {g.tasks.length === 1 ? "task" : "tasks"}
                  </span>
                  <div className="ml-auto flex gap-[7px]">
                    {g.tasks.some((t) => t.action === "Copy") && (
                      <button
                        onClick={() => void runGroup(g)}
                        className="cursor-pointer whitespace-nowrap rounded-full bg-nexus-accent px-[13px] py-[5px] text-[11.5px] font-bold text-white hover:bg-nexus-accent-hover"
                      >
                        Run group
                      </button>
                    )}
                    <button
                      onClick={() => toast(`Add task to ${g.name}`)}
                      className="cursor-pointer whitespace-nowrap rounded-full border border-nexus-border2 bg-nexus-bg px-3 py-[5px] text-[11.5px] font-semibold text-[#7a6f60] hover:bg-[#ece2d5]"
                    >
                      Add task
                    </button>
                  </div>
                </div>

                <div
                  className="grid items-center gap-3 bg-nexus-sand2 px-5 py-[9px] text-[10px] font-bold uppercase tracking-[.05em] text-[#c3b9a8]"
                  style={{ gridTemplateColumns: TASK_COLS }}
                >
                  <div />
                  <div>Type</div>
                  <div>Source</div>
                  <div>Target</div>
                  <div className="text-right">Actions</div>
                </div>

                {g.tasks.map((t) => {
                  const isCopy = t.action === "Copy";
                  const st = isCopy ? statusOf(t.status) : null;
                  const tDragging = dragTask != null && dragTask.groupId === g.id && dragTask.taskId === t.id;
                  const targetLabel = t.target || "—";
                  return (
                    <div
                      key={t.id}
                      draggable
                      onDragStart={(e) => {
                        setDragTask({ groupId: g.id, taskId: t.id });
                        e.dataTransfer.effectAllowed = "move";
                        try { e.dataTransfer.setData("text/plain", t.id); } catch { /* noop */ }
                        e.stopPropagation();
                      }}
                      onDragOver={(e) => { if (dragTask?.groupId === g.id) { e.preventDefault(); e.stopPropagation(); } }}
                      onDrop={(e) => { if (dragTask && dragTask.groupId === g.id) { e.preventDefault(); e.stopPropagation(); reorderTask(g.id, dragTask.taskId, t.id); setDragTask(null); } }}
                      onDragEnd={() => setDragTask(null)}
                      className={cn("grid items-center gap-3 border-t border-[#f3eee5] px-5 py-3", tDragging && "bg-[#fbf6ef] opacity-50")}
                      style={{ gridTemplateColumns: TASK_COLS }}
                    >
                      <span title="Drag to reorder task" className="cursor-grab text-[12px] leading-none tracking-[-1px] text-[#d4c9b6]">
                        ⠿
                      </span>
                      <div className="flex min-w-0 flex-col gap-[3px]">
                        <DirBadge direction={t.direction} />
                        <span className="font-mono text-[10px] text-[#b3a999]">{t.action}</span>
                      </div>
                      <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#6a6055]">
                        {t.source}
                      </div>
                      <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#8a8073]">
                        {targetLabel}
                      </div>
                      <div className="flex items-center justify-end gap-[9px]">
                        {st && (
                          <span className="inline-flex items-center gap-[5px] text-[11px] font-bold" style={{ color: st.fg }}>
                            <Dot color={st.dot} /> {st.label}
                          </span>
                        )}
                        {isCopy && (
                          <>
                            <span
                              onClick={() =>
                                setSched({
                                  groupId: g.id,
                                  taskId: t.id,
                                  taskName: `${t.direction} · ${t.source || "task"}`,
                                  mode: t.schedule !== "manual" ? "cron" : "manual",
                                  cronExpr: t.schedule !== "manual" ? t.schedule : "0 5 * * *",
                                })
                              }
                              className="cursor-pointer text-[11px] font-bold text-nexus-accent hover:underline"
                            >
                              Schedule
                            </span>
                            <span
                              onClick={() => void runTask(g.id, t)}
                              className="cursor-pointer text-[11px] font-bold text-nexus-accent hover:underline"
                            >
                              Run
                            </span>
                          </>
                        )}
                        <span
                          onClick={() => void deleteTask(g.id, t)}
                          className="cursor-pointer text-[11px] font-bold text-nexus-crit hover:underline"
                        >
                          Delete
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            );
          })}
          {!taskGroupsQuery.isLoading && groups.length === 0 ? (
            <div className="rounded-[18px] border border-dashed border-nexus-border2 bg-nexus-card px-6 py-10 text-center">
              <div className="text-[14px] font-bold text-[#7a6f60]">No task groups yet</div>
              <div className="mt-1.5 text-[12.5px] text-[#b3a999]">
                Create a custom task to start a group. Templates live inside the create flow.
              </div>
            </div>
          ) : null}
        </div>

        <div className="mt-8 flex flex-wrap items-center gap-2.5">
          <h2 className="m-0 whitespace-nowrap text-[15px] font-extrabold text-nexus-ink">
            Project Symlinks
          </h2>
          <span className="text-[11px] text-[#b3a999]">
            Auto-scanned from registered Project paths · target path is the symlink placement
          </span>
          <Button
            variant="subtle"
            size="sm"
            className="ml-auto"
            disabled={projectSymlinksQuery.isFetching}
            onClick={() => void projectSymlinksQuery.refetch()}
          >
            <RefreshCw size={14} className={cn(projectSymlinksQuery.isFetching && "animate-spin")} />
            {projectSymlinksQuery.isFetching ? "Refreshing..." : "Refresh"}
          </Button>
        </div>

        <div className="mt-3 overflow-hidden rounded-[18px] border border-nexus-border bg-nexus-card shadow-[0_1px_14px_rgba(50,40,25,.05)]">
          <div
            className="grid items-center gap-3 bg-nexus-sand2 px-5 py-[9px] text-[10px] font-bold uppercase tracking-[.05em] text-[#c3b9a8]"
            style={{ gridTemplateColumns: LINK_COLS }}
          >
            <div>Source Project</div>
            <div>Source Path</div>
            <div>Target Project</div>
            <div>Target Path</div>
            <div className="text-right">Actions</div>
          </div>
          {projectSymlinksQuery.isLoading ? (
            <div className="flex items-center gap-2 px-5 py-6 text-[12.5px] text-[#9a8f80]">
              <Spinner /> Scanning Project symlinks...
            </div>
          ) : projectSymlinkError ? (
            <div className="px-5 py-6 text-[12.5px] font-semibold text-nexus-crit">
              {projectSymlinkError}
            </div>
          ) : projectSymlinks.length === 0 ? (
            <div className="px-5 py-6">
              <div className="text-[13px] font-bold text-[#7a6f60]">No Project symlinks found</div>
              <div className="mt-1 text-[12px] text-[#b3a999]">
                Registered Project directories do not currently contain symlink placements.
              </div>
            </div>
          ) : (
            projectSymlinks.map((link) => {
              return (
                <div
                  key={link.id}
                  className="grid items-center gap-3 border-t border-[#f3eee5] px-5 py-3"
                  style={{ gridTemplateColumns: LINK_COLS }}
                >
                  <div className="min-w-0">
                    <div className="overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] font-bold text-nexus-body">
                      {link.sourceProjectName ?? "External"}
                    </div>
                    <div className="mt-[3px] text-[10px] font-bold uppercase tracking-[.04em] text-[#c3b9a8]">
                      {link.linkKind}
                    </div>
                  </div>
                  <div className="flex min-w-0 items-center gap-1.5">
                    <span className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#8a8073]" title={link.sourcePath}>
                      {formatProjectSymlinkDisplayPath(link.sourcePath, link.sourceProjectName)}
                    </span>
                    <CopyPathButton path={link.sourcePath} />
                  </div>
                  <div className="overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] font-bold text-nexus-body">
                    {link.targetProjectName ?? "External"}
                  </div>
                  <div className="flex min-w-0 items-center gap-1.5">
                    <span className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#8a8073]" title={link.targetPath}>
                      {formatProjectSymlinkDisplayPath(link.targetPath, link.targetProjectName)}
                    </span>
                    <CopyPathButton path={link.targetPath} />
                  </div>
                  <div className="justify-self-end">
                    <span
                      onClick={() => void deleteProjectSymlink(link.targetPath)}
                      className="cursor-pointer text-[11px] font-bold text-nexus-crit hover:underline"
                    >
                      Delete
                    </span>
                  </div>
                </div>
              );
            })
          )}
        </div>

        <div className="mt-8 flex items-center gap-2.5">
          <h2 className="m-0 text-[13px] font-bold tracking-[.02em] text-[#9a8f80]">
            System-managed records
          </h2>
          <span className="text-[11px] text-[#c3b9a8]">
            Default behaviors · generated from Skill / Prompt / Session — collapsed by default
          </span>
        </div>

        <div className="mt-3 flex flex-col gap-2.5">
          {sysSections.map((sec) => {
            const open = openSec[sec.key];
            return (
              <div key={sec.key} className="overflow-hidden rounded-[14px] border border-nexus-border bg-nexus-sand2">
                <div
                  onClick={() => setOpenSec((s) => ({ ...s, [sec.key]: !s[sec.key] }))}
                  className="flex cursor-pointer items-center gap-[11px] px-[18px] py-[13px] hover:bg-[#f4ede1]"
                >
                  <span
                    className="inline-block text-[10px] text-[#a99a89] transition-transform"
                    style={{ transform: open ? "rotate(90deg)" : "rotate(0deg)" }}
                  >
                    ▸
                  </span>
                  <span className="text-[13.5px] font-bold text-[#6a6055]">{sec.title}</span>
                  <span className="rounded-[6px] bg-[rgba(157,122,100,.12)] px-2 py-0.5 text-[10px] font-bold text-nexus-accent">
                    {sec.managedBy}
                  </span>
                  <span className="ml-auto text-[11.5px] text-[#b3a999]">{sec.count}</span>
                </div>
                {open ? (
                  <div className="border-t border-nexus-border bg-nexus-card">
                    <div
                      className="grid gap-3.5 bg-nexus-sand2 px-[18px] py-[9px] text-[10px] font-bold uppercase tracking-[.05em] text-[#c3b9a8]"
                      style={{ gridTemplateColumns: "1.2fr 1.5fr 1.6fr 120px" }}
                    >
                      <div>Asset</div>
                      <div>Relation</div>
                      <div>Target path</div>
                      <div className="text-right">Status</div>
                    </div>
                    {sec.rows.map((r, i) => {
                      const st = statusOf(r.status);
                      return (
                        <div
                          key={i}
                          className="grid items-center gap-3.5 border-t border-[#f3eee5] px-[18px] py-[11px]"
                          style={{ gridTemplateColumns: "1.2fr 1.5fr 1.6fr 120px" }}
                        >
                          <div className="text-[12.5px] font-bold text-nexus-body">{r.asset}</div>
                          <div className="text-[11.5px] text-[#6a6055]">{r.relation}</div>
                          <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#a99a89]">
                            {r.path}
                          </div>
                          <div className="inline-flex items-center gap-1.5 justify-self-end text-[11px] font-bold" style={{ color: st.fg }}>
                            <Dot color={st.dot} /> {st.label}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </ScreenScroll>

      {/* Create custom task modal */}
      <Modal open={createOpen} onClose={() => setCreateOpen(false)} className="max-h-[90vh] w-[640px]">
        <ModalHeader
          title="Create custom task"
          subtitle="A Task Group holds one or more tasks. Direction, action & schedule are set per task."
        />
        <div className="flex flex-col gap-5 px-[22px] py-5">
          <div>
            <div className="mb-2.5 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
              Start from a template
            </div>
            <div className="flex flex-wrap gap-2">
              {templates.map((t) => (
                <Chip key={t.id} active={tpl === t.id} onClick={() => pickTemplate(t.id)} title={t.desc}>
                  {t.name}
                  <span className="font-semibold opacity-70"> · {t.tasks.length} {t.tasks.length === 1 ? "task" : "tasks"}</span>
                </Chip>
              ))}
            </div>
          </div>

          <label className="block">
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Task group name</div>
            <Input
              className="text-[13px]"
              placeholder="e.g. Machine Backup"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            />
          </label>

          <div>
            <div className="mb-2 flex items-center justify-between">
              <div className="text-[12px] font-bold text-[#6a6055]">
                Tasks <span className="font-medium text-[#b3a999]">· {form.tasks.length} {form.tasks.length === 1 ? "task" : "tasks"}</span>
              </div>
              <div
                onClick={() => setForm((f) => ({ ...f, tasks: [...f.tasks, newTask()] }))}
                className="cursor-pointer text-[11.5px] font-bold text-nexus-accent hover:underline"
              >
                + Add task
              </div>
            </div>
            <div className="flex flex-col gap-3">
              {form.tasks.map((tk, i) => {
                const isCron = tk.schedule !== "manual";
                return (
                  <div key={tk.id} className="rounded-[14px] border border-nexus-border bg-nexus-sand2 p-3.5">
                    <div className="mb-[11px] flex items-center justify-between gap-2.5">
                      <span className="text-[11px] font-bold text-nexus-accent">Task {i + 1}</span>
                      {form.tasks.length > 1 ? (
                        <div
                          onClick={() => setForm((f) => ({ ...f, tasks: f.tasks.filter((_, j) => j !== i) }))}
                          className="cursor-pointer text-[11px] font-semibold text-nexus-crit hover:underline"
                        >
                          Remove
                        </div>
                      ) : null}
                    </div>
                    <div className="mb-[11px] flex flex-wrap gap-4">
                      <div>
                        <div className="mb-[5px] text-[11px] font-semibold text-[#8a7d6c]">Action</div>
                        <Segmented<TaskAction>
                          className="bg-[#ece2d5]"
                          size="sm"
                          options={[
                            { value: "Symlink", label: "Symlink", disabled: tk.targets.some((t) => t.type === "Cloud") || tk.sourceType === "Cloud" },
                            { value: "Copy", label: "Copy" },
                          ]}
                          value={tk.action}
                          onChange={(a) => patchFormTask(i, { action: a, schedule: a === "Symlink" ? "manual" : tk.schedule })}
                        />
                      </div>
                    </div>
                    <div className="mb-2.5">
                      <div className="mb-[5px] text-[11px] font-semibold text-[#8a7d6c]">
                        Source <span className="font-medium text-[#b3a999]">(single)</span>
                      </div>
                      <div className="flex items-center gap-[7px]">
                        <Segmented<LocationType>
                          className="bg-[#ece2d5]"
                          size="sm"
                          options={[{ value: "Local", label: "Local" }, { value: "Cloud", label: "Cloud" }]}
                          value={tk.sourceType}
                          onChange={(v) => patchFormTask(i, { sourceType: v })}
                        />
                        <Input
                          className="flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
                          placeholder={tk.sourceType === "Cloud" ? "config/warp/" : "~/.config/warp/"}
                          value={tk.source}
                          onChange={(e) => patchFormTask(i, { source: e.target.value })}
                        />
                      </div>
                    </div>
                    <div className="mb-2.5">
                      <div className="mb-[5px] flex items-center justify-between">
                        <div className="text-[11px] font-semibold text-[#8a7d6c]">
                          Targets <span className="font-medium text-[#b3a999]">(one or more)</span>
                        </div>
                        <div
                          onClick={() => patchFormTask(i, { targets: [...tk.targets, { type: "Local", path: "" }] })}
                          className="cursor-pointer text-[11px] font-bold text-nexus-accent hover:underline"
                        >
                          + Add
                        </div>
                      </div>
                      <div className="flex flex-col gap-[7px]">
                        {tk.targets.map((val, j) => (
                          <div key={j} className="flex items-center gap-[7px]">
                            <Segmented<LocationType>
                              className="bg-[#ece2d5]"
                              size="sm"
                              options={[{ value: "Local", label: "Local" }, { value: "Cloud", label: "Cloud" }]}
                              value={val.type}
                              onChange={(v) => {
                                const tg = [...tk.targets];
                                tg[j] = { ...tg[j], type: v };
                                patchFormTask(i, { targets: tg });
                              }}
                            />
                            <Input
                              className="flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
                              placeholder={val.type === "Cloud" ? "backups/ssh/" : "/target/path/"}
                              value={val.path}
                              onChange={(e) => {
                                const tg = [...tk.targets];
                                tg[j] = { ...tg[j], path: e.target.value };
                                patchFormTask(i, { targets: tg });
                              }}
                            />
                            <div
                              onClick={() => {
                                const tg = tk.targets.filter((_, k) => k !== j);
                                patchFormTask(i, { targets: tg.length ? tg : [{ type: "Local", path: "" }] });
                              }}
                              className="inline-flex h-[30px] w-[30px] flex-none cursor-pointer items-center justify-center rounded-[8px] border border-nexus-border2 bg-white text-[15px] text-[#b3a999] hover:bg-nexus-bg hover:text-nexus-crit"
                            >
                              −
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                    {tk.action === "Copy" ? (
                      <div>
                        <div className="mb-[5px] text-[11px] font-semibold text-[#8a7d6c]">Schedule</div>
                        <div className="flex flex-wrap items-center gap-2">
                          <Segmented<"manual" | "cron">
                            className="bg-[#ece2d5]"
                            size="sm"
                            options={[
                              { value: "manual", label: "Manual" },
                              { value: "cron", label: "Schedule" },
                            ]}
                            value={isCron ? "cron" : "manual"}
                            onChange={(v) => patchFormTask(i, { schedule: v === "manual" ? "manual" : isCron ? tk.schedule : "0 5 * * *" })}
                          />
                          {isCron ? (
                            <Input
                              className="min-w-[120px] flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
                              placeholder="0 5 * * *"
                              value={tk.schedule}
                              onChange={(e) => patchFormTask(i, { schedule: e.target.value || " " })}
                            />
                          ) : null}
                        </div>
                        {isCron ? (
                          <div className="mt-[7px] flex flex-wrap gap-1.5">
                            {SCHEDULE_PRESETS.map((cp) => (
                              <Chip key={cp.expr} mono active={tk.schedule === cp.expr} onClick={() => patchFormTask(i, { schedule: cp.expr })}>
                                {cp.expr}
                              </Chip>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    ) : null}
                  </div>
                );
              })}
            </div>
          </div>

          <div className="rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-[11px] text-[11.5px] leading-[1.5] text-[#8a7a68]">
            Every task is one-way · single source → one or more targets. To bring files back from
            the Cloud, add a <b className="text-[#6a6055]">Restore/Pull</b> task — Backup is never
            reversed in place.
          </div>
        </div>
        <ModalFooter>
          <Button variant="subtle" onClick={() => setCreateOpen(false)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submitCreate}>
            Create task group
          </Button>
        </ModalFooter>
      </Modal>

      {/* Edit schedule modal */}
      <Modal open={!!sched} onClose={() => setSched(null)} className="w-[440px]" overlayClassName="z-[70]">
        {sched ? (
          <>
            <ModalHeader
              title={`Schedule · ${sched.taskName}`}
              titleClassName="text-[16px]"
              subtitle="Per-task trigger. Scheduled runs are not implemented yet."
            />
            <div className="flex flex-col gap-3.5 px-[22px] py-5">
              <Segmented<"manual" | "cron">
                className="w-fit"
                size="sm"
                options={[
                  { value: "manual", label: "Manual" },
                  { value: "cron", label: "Schedule" },
                ]}
                value={sched.mode}
                onChange={(v) => setSched((s) => (s ? { ...s, mode: v } : s))}
              />
              {sched.mode === "cron" ? (
                <div>
                  <Input
                    className="font-mono"
                    placeholder="0 5 * * *"
                    value={sched.cronExpr}
                    onChange={(e) => setSched((s) => (s ? { ...s, cronExpr: e.target.value } : s))}
                  />
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {SCHEDULE_PRESETS.map((cp) => (
                      <Chip key={cp.expr} mono active={sched.cronExpr === cp.expr} onClick={() => setSched((s) => (s ? { ...s, cronExpr: cp.expr } : s))}>
                        {cp.expr}
                      </Chip>
                    ))}
                  </div>
                  <div className="mt-2.5 text-[11px] text-[#b3a999]">{cronHuman(sched.cronExpr)}</div>
                </div>
              ) : (
                <div className="rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-3 text-[12px] leading-[1.5] text-[#8a7a68]">
                  Manual — this task only runs when you trigger it from the group or the task row.
                </div>
              )}
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => setSched(null)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  const isCron = sched.mode === "cron";
                  const val = isCron ? sched.cronExpr.trim() || "manual" : "manual";
                  updateTask(sched.groupId, sched.taskId, { schedule: val });
                  setSched(null);
                  toast(isCron ? `Schedule set · ${val}` : "Schedule set to Manual");
                }}
              >
                Save schedule
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>
    </>
  );
}
