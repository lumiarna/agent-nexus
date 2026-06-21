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
import { useQueryClient } from "@tanstack/react-query";
import { Copy, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dot, Input, Spinner } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Chip, Segmented } from "@/components/ui/segmented";
import { ScreenScroll } from "@/components/shell/screen";
import { formatProjectSymlinkDisplayPath } from "@/components/sync/pathDisplay";
import { DEFAULT_CRON_SCHEDULE, SCHEDULE_PRESETS, cronHuman } from "@/components/sync/schedule";
import {
  actionOptions,
  expandFormTask,
  isCronSchedule,
  normalizeSchedule,
  scheduleForAction,
  scheduleForMode,
} from "@/components/sync/taskRules";
import { detectPlatform, type HostPlatform } from "@/lib/runtime";
import {
  useAddTaskMutation,
  useCreateTaskGroupMutation,
  useDeleteTaskGroupMutation,
  useDeleteTaskMutation,
  useRunTaskMutation,
  useTaskGroupsQuery,
  useUpdateTaskScheduleMutation,
  syncKeys,
} from "@/lib/query/sync";
import {
  useDeleteProjectSymlinkMutation,
  useProjectSymlinksQuery,
} from "@/lib/query/projectSymlinks";
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

// Both tables use a 16fr grid and must keep their Action column aligned. A column's x is
// `sum(fr before it) + sum(gaps before it)`, so alignment needs BOTH the leading fr (7) AND
// the leading cell-count (→ gap-count) to match. Each table therefore has exactly 2 cells
// before Action: Task folds its drag handle into the Direction cell so it isn't a 3rd column.
const TASK_COLS = "1fr 6fr 0.8fr 6fr 2.2fr";
const LINK_COLS = "1.4fr 5.6fr 1fr 1.4fr 5.6fr 1fr";

function actionColors(a: TaskAction): { fg: string; bg: string } {
  if (a === "Junction") return { fg: "#6f5b92", bg: "#ebe5f2" };
  if (a === "Copy") return { fg: "#9a6f0a", bg: "#f7eccb" };
  return { fg: "#4a6a8a", bg: "#e2ebf2" };
}

function directionColor(d: TaskDirection): string {
  if (d === "Push") return "#9a6f0a";
  if (d === "Pull") return "#3f6f55";
  return "#4a6a8a";
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

function ActionBadge({ action }: { action: TaskAction }) {
  const c = actionColors(action);
  return (
    <span
      className="inline-flex w-fit items-center justify-center rounded-[5px] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.03em]"
      style={{ color: c.fg, background: c.bg }}
    >
      {action}
    </span>
  );
}

function LocationTag({ type }: { type: LocationType }) {
  const isCloud = type === "Cloud";
  return (
    <span
      className="inline-flex flex-none items-center rounded-[4px] px-[5px] py-[1px] text-[9px] font-bold uppercase tracking-[.04em]"
      style={{
        color: isCloud ? "#4a6a8a" : "#8a7d6c",
        background: isCloud ? "#e2ebf2" : "#efe7da",
      }}
    >
      {type}
    </span>
  );
}

/** Selectable, monospaced block for a submit failure — keeps multi-line guidance (e.g. the
 *  mklink command) readable and copyable instead of flashing past in a toast. */
function SubmitError({ message }: { message: string }) {
  return (
    <div className="select-text whitespace-pre-wrap break-words rounded-[11px] border border-[#e3b8af] bg-[#fbecea] px-3.5 py-3 font-mono text-[11.5px] leading-[1.6] text-nexus-crit">
      {message}
    </div>
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

type SortableRender = ReturnType<typeof useSortable>;

function taskKey(groupId: string, taskId: string): string {
  return `${groupId}::${taskId}`;
}

function SortableTaskGroup({ id, children }: {
  id: string;
  children: (s: SortableRender) => ReactNode;
}) {
  const sortable = useSortable({ id, data: { type: "group" } });
  return <>{children(sortable)}</>;
}

function SortableTask({ groupId, taskId, children }: {
  groupId: string;
  taskId: string;
  children: (s: SortableRender) => ReactNode;
}) {
  const sortable = useSortable({
    id: taskKey(groupId, taskId),
    data: { type: "task", groupId, taskId },
  });
  return <>{children(sortable)}</>;
}

export function SyncPage() {
  const queryClient = useQueryClient();
  const taskGroupsQuery = useTaskGroupsQuery();
  const createTaskGroupMutation = useCreateTaskGroupMutation();
  const deleteTaskMutation = useDeleteTaskMutation();
  const deleteTaskGroupMutation = useDeleteTaskGroupMutation();
  const addTaskMutation = useAddTaskMutation();
  const runTaskMutation = useRunTaskMutation();
  const updateTaskScheduleMutation = useUpdateTaskScheduleMutation();
  const projectSymlinksQuery = useProjectSymlinksQuery();
  const deleteProjectSymlinkMutation = useDeleteProjectSymlinkMutation();
  const [templates] = useState(() => nexus.templates());
  const [system] = useState(() => nexus.systemSync());
  const [openSec, setOpenSec] = useState({ skill: false, prompt: false, backup: false });
  const [createOpen, setCreateOpen] = useState(false);
  const [tpl, setTpl] = useState("blank");
  const [form, setForm] = useState<FormState>({ name: "", tasks: [newTask()] });
  const [sched, setSched] = useState<SchedState | null>(null);
  const [addTarget, setAddTarget] = useState<TaskGroup | null>(null);
  const [addForm, setAddForm] = useState<FormTask | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<TaskGroup | null>(null);
  const [deleteTaskTarget, setDeleteTaskTarget] = useState<{ group: TaskGroup; task: Task } | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);
  const [addError, setAddError] = useState<string | null>(null);
  const [platform, setPlatform] = useState<HostPlatform>("unknown");
  const supportsJunction = platform === "windows";
  const projectSymlinks = projectSymlinksQuery.data ?? [];
  const projectSymlinkError = projectSymlinksQuery.error
    ? getErrorMessage(projectSymlinksQuery.error)
    : null;
  const groups = taskGroupsQuery.data ?? [];

  useEffect(() => {
    void detectPlatform().then(setPlatform);
  }, []);

  // ── mutations ──
  function updateTaskGroups(updater: (groups: TaskGroup[]) => TaskGroup[]) {
    queryClient.setQueryData<TaskGroup[]>(syncKeys.taskGroups, (current) =>
      updater(current ?? []),
    );
  }

  function updateTask(groupId: string, taskId: string, patch: Partial<Task>) {
    updateTaskGroups((gs) =>
      gs.map((g) =>
        g.id !== groupId
          ? g
          : { ...g, tasks: g.tasks.map((t) => (t.id !== taskId ? t : { ...t, ...patch })) },
      ),
    );
  }

  function reorderGroups(fromId: string, toId: string) {
    if (!fromId || fromId === toId) return;
    updateTaskGroups((gs) => {
      const fi = gs.findIndex((g) => g.id === fromId);
      const ti = gs.findIndex((g) => g.id === toId);
      if (fi < 0 || ti < 0) return gs;
      return arrayMove(gs, fi, ti);
    });
  }
  function reorderTask(groupId: string, fromId: string, toId: string) {
    if (!fromId || fromId === toId) return;
    updateTaskGroups((gs) =>
      gs.map((g) => {
        if (g.id !== groupId) return g;
        const fi = g.tasks.findIndex((t) => t.id === fromId);
        const ti = g.tasks.findIndex((t) => t.id === toId);
        if (fi < 0 || ti < 0) return g;
        return { ...g, tasks: arrayMove(g.tasks, fi, ti) };
      }),
    );
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  function handleDragEnd(e: DragEndEvent) {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const a = active.data.current;
    const o = over.data.current;
    if (a?.type === "group" && o?.type === "group") {
      reorderGroups(String(active.id), String(over.id));
    } else if (
      a?.type === "task" &&
      o?.type === "task" &&
      a.groupId === o.groupId
    ) {
      reorderTask(a.groupId, a.taskId, o.taskId);
    }
  }

  async function deleteTask(groupId: string, task: Task) {
    try {
      await deleteTaskMutation.mutateAsync(task.id);
      updateTaskGroups((gs) =>
        gs.map((g) =>
          g.id === groupId ? { ...g, tasks: g.tasks.filter((t) => t.id !== task.id) } : g,
        ),
      );
      toast(`Deleted · ${task.direction} · ${task.source || "task"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function confirmDeleteTask() {
    if (!deleteTaskTarget) return;
    const { group, task } = deleteTaskTarget;
    setDeleteTaskTarget(null);
    await deleteTask(group.id, task);
  }

  async function confirmDeleteTaskGroup() {
    if (!deleteTarget) return;
    const group = deleteTarget;
    try {
      await deleteTaskGroupMutation.mutateAsync(group.id);
      updateTaskGroups((gs) => gs.filter((g) => g.id !== group.id));
      setDeleteTarget(null);
      toast(`Deleted group · ${group.name}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  function openAddTask(group: TaskGroup) {
    setAddTarget(group);
    setAddForm(newTask());
    setAddError(null);
  }

  async function submitAddTask() {
    if (!addTarget || !addForm) return;
    const tk = addForm;
    const drafts = expandFormTask(tk);
    setAddError(null);
    if (!drafts.length || !tk.source.trim()) {
      setAddError("Source and at least one target are required");
      return;
    }
    try {
      for (const task of drafts) {
        await addTaskMutation.mutateAsync({ groupId: addTarget.id, task });
      }
      setAddTarget(null);
      setAddForm(null);
      toast(`Task added to · ${addTarget.name}`);
    } catch (error) {
      setAddError(getErrorMessage(error));
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

  async function saveSchedule() {
    if (!sched) return;
    const isCron = sched.mode === "cron";
    const schedule = isCron ? sched.cronExpr.trim() || "manual" : "manual";
    try {
      const updated = await updateTaskScheduleMutation.mutateAsync({
        id: sched.taskId,
        schedule,
      });
      updateTask(sched.groupId, sched.taskId, updated);
      setSched(null);
      toast(isCron ? `Schedule set · ${schedule}` : "Schedule set to Manual");
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
    setCreateError(null);
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
          ? t.tasks.map((tk) => ({ ...newTask(), action: tk.action, sourceType: tk.sourceType, source: tk.source, targets: [{ type: tk.targetType, path: tk.target }], schedule: tk.action === "Copy" ? tk.schedule : "manual" }))
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
          schedule: normalizeSchedule(tk.schedule),
      }));
    });
    setCreateError(null);
    try {
      const created = await createTaskGroupMutation.mutateAsync({ name, tasks });
      updateTaskGroups((gs) => [...gs.filter((g) => g.id !== created.id), created]);
      setCreateOpen(false);
      toast(`Task group "${name}" created · ${tasks.length} ${tasks.length === 1 ? "task" : "tasks"}`);
    } catch (error) {
      setCreateError(getErrorMessage(error));
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
          {!taskGroupsQuery.isLoading && groups.length > 0 ? (
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleDragEnd}
            >
              <SortableContext
                items={groups.map((g) => g.id)}
                strategy={verticalListSortingStrategy}
              >
                {groups.map((g) => (
                  <SortableTaskGroup key={g.id} id={g.id}>
                    {({
                      setNodeRef,
                      setActivatorNodeRef,
                      listeners,
                      attributes,
                      transform,
                      transition,
                      isDragging,
                    }) => (
                      <div
                        ref={setNodeRef}
                        className={cn(
                          "overflow-hidden rounded-[18px] border bg-nexus-card transition-[box-shadow,opacity]",
                          isDragging
                            ? "border-nexus-accent opacity-60 shadow-[0_8px_28px_rgba(50,40,25,.16)]"
                            : "border-nexus-border shadow-[0_1px_14px_rgba(50,40,25,.05)]",
                        )}
                        style={{
                          transform: CSS.Transform.toString(transform),
                          transition,
                        }}
                        {...attributes}
                      >
                        <div className="flex flex-wrap items-center gap-[11px] border-b border-[#f3eee5] px-5 py-[15px]">
                          <span
                            ref={setActivatorNodeRef}
                            {...listeners}
                            title="Drag to reorder group"
                            className="cursor-grab text-[13px] leading-none tracking-[-1px] text-[#cabfae]"
                          >
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
                              onClick={() => openAddTask(g)}
                              className="cursor-pointer whitespace-nowrap rounded-full border border-nexus-border2 bg-nexus-bg px-3 py-[5px] text-[11.5px] font-semibold text-[#7a6f60] hover:bg-[#ece2d5]"
                            >
                              Add task
                            </button>
                            <button
                              onClick={() => setDeleteTarget(g)}
                              className="cursor-pointer whitespace-nowrap rounded-full border border-nexus-border2 bg-nexus-bg px-3 py-[5px] text-[11.5px] font-semibold text-nexus-crit hover:bg-[#f6e3e0]"
                            >
                              Delete group
                            </button>
                          </div>
                        </div>

                        <div
                          className="grid items-center gap-3 bg-nexus-sand2 px-5 py-[9px] text-[10px] font-bold uppercase tracking-[.05em] text-[#c3b9a8]"
                          style={{ gridTemplateColumns: TASK_COLS }}
                        >
                          <div>Direction</div>
                          <div>Source</div>
                          <div>Action</div>
                          <div>Target</div>
                          <div className="text-right">Manage</div>
                        </div>

                        <SortableContext
                          items={g.tasks.map((t) => taskKey(g.id, t.id))}
                          strategy={verticalListSortingStrategy}
                        >
                          {g.tasks.map((t) => {
                            const isCopy = t.action === "Copy";
                            const st = isCopy ? statusOf(t.status) : null;
                            const linkMissing = !isCopy && t.linkState === "missing";
                            const targetLabel = t.target || "—";
                            return (
                              <SortableTask key={t.id} groupId={g.id} taskId={t.id}>
                                {({
                                  setNodeRef,
                                  setActivatorNodeRef,
                                  listeners,
                                  attributes,
                                  transform,
                                  transition,
                                  isDragging,
                                }) => (
                                  <div
                                    ref={setNodeRef}
                                    className={cn(
                                      "grid items-center gap-3 border-t border-[#f3eee5] px-5 py-3",
                                      isDragging && "bg-[#fbf6ef] opacity-50",
                                      linkMissing && "bg-[#fbf3f0]",
                                    )}
                                    style={{
                                      gridTemplateColumns: TASK_COLS,
                                      transform: CSS.Transform.toString(transform),
                                      transition,
                                    }}
                                    {...attributes}
                                  >
                                    <div className="flex items-center gap-2">
                                      <span
                                        ref={setActivatorNodeRef}
                                        {...listeners}
                                        title="Drag to reorder task"
                                        className="cursor-grab text-[12px] leading-none tracking-[-1px] text-[#d4c9b6]"
                                      >
                                        ⠿
                                      </span>
                                      <span
                                        className="text-[9.5px] font-bold uppercase tracking-[.03em]"
                                        style={{ color: directionColor(t.direction) }}
                                        title={t.direction}
                                      >
                                        {t.direction}
                                      </span>
                                    </div>
                                    <div className="flex min-w-0 items-center gap-1.5">
                                      <LocationTag type={t.sourceType} />
                                      <span className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#6a6055]">
                                        {t.source}
                                      </span>
                                    </div>
                                    <div className="flex items-center" title={t.action}>
                                      <ActionBadge action={t.action} />
                                    </div>
                                    <div className="flex min-w-0 items-center gap-1.5">
                                      <LocationTag type={t.targetType} />
                                      <span className={cn(
                                        "overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px]",
                                        linkMissing ? "text-[#b55440] line-through" : "text-[#8a8073]",
                                      )} title={linkMissing ? "Placement missing — symlink/junction was removed out-of-band" : undefined}>
                                        {targetLabel}
                                      </span>
                                    </div>
                                    <div className="flex items-center justify-end gap-[9px]">
                                      {st && (
                                        <span className="inline-flex items-center gap-[5px] text-[11px] font-bold" style={{ color: st.fg }}>
                                          <Dot color={st.dot} /> {st.label}
                                        </span>
                                      )}
                                      {linkMissing && (
                                        <span className="inline-flex items-center gap-[5px] text-[11px] font-bold" style={{ color: palette.crit }}>
                                          <Dot color={palette.crit} /> Missing
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
                                                cronExpr: t.schedule !== "manual" ? t.schedule : DEFAULT_CRON_SCHEDULE,
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
                                        onClick={() => setDeleteTaskTarget({ group: g, task: t })}
                                        className="cursor-pointer text-[11px] font-bold text-nexus-crit hover:underline"
                                      >
                                        Delete
                                      </span>
                                    </div>
                                  </div>
                                )}
                              </SortableTask>
                            );
                          })}
                        </SortableContext>
                      </div>
                    )}
                  </SortableTaskGroup>
                ))}
              </SortableContext>
            </DndContext>
          ) : null}
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
            Auto-scanned from registered Project paths · symlinks already managed by a task above are hidden here
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
            <div>Action</div>
            <div>Target Project</div>
            <div>Target Path</div>
            <div className="text-right">Manage</div>
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
                  </div>
                  <div className="flex min-w-0 items-center gap-1.5">
                    <span className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#8a8073]" title={link.sourcePath}>
                      {formatProjectSymlinkDisplayPath(link.sourcePath, link.sourceProjectName)}
                    </span>
                    <CopyPathButton path={link.sourcePath} />
                  </div>
                  <div title={link.linkType}>
                    <ActionBadge action={link.linkType} />
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
                const isCron = isCronSchedule(tk.schedule);
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
                          options={actionOptions(
                            { sourceType: tk.sourceType, targets: tk.targets },
                            supportsJunction,
                          )}
                          value={tk.action}
                          onChange={(a) => patchFormTask(i, { action: a, schedule: scheduleForAction(a, tk.schedule) })}
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
                            onChange={(v) => patchFormTask(i, { schedule: scheduleForMode(v, tk.schedule, DEFAULT_CRON_SCHEDULE) })}
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
          {createError ? <SubmitError message={createError} /> : null}
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
                onClick={() => void saveSchedule()}
              >
                Save schedule
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>

      {/* Add task modal */}
      <Modal
        open={!!addTarget}
        onClose={() => { setAddTarget(null); setAddForm(null); }}
        className="max-h-[90vh] w-[640px]"
        overlayClassName="z-[70]"
      >
        {addTarget && addForm ? (
          <>
            <ModalHeader
              title={`Add task to · ${addTarget.name}`}
              subtitle="A new task is appended to the end of this group."
            />
            <div className="flex flex-col gap-5 px-[22px] py-5">
              <AddTaskForm
                task={addForm}
                onChange={setAddForm}
                supportsJunction={supportsJunction}
              />
              <div className="rounded-[11px] border border-nexus-border bg-nexus-bg px-3.5 py-[11px] text-[11.5px] leading-[1.5] text-[#8a7a68]">
                Every task is one-way · single source → one or more targets.
              </div>
              {addError ? <SubmitError message={addError} /> : null}
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => { setAddTarget(null); setAddForm(null); }}>
                Cancel
              </Button>
              <Button variant="primary" onClick={() => void submitAddTask()}>
                Add task
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>

      {/* Delete task group confirm */}
      <Modal
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        className="w-[440px]"
        overlayClassName="z-[70]"
      >
        {deleteTarget ? (
          <>
            <ModalHeader
              title="Delete task group"
              titleClassName="text-[16px]"
              subtitle="This also removes any local symlink / junction placements this group created."
            />
            <div className="px-[22px] py-5 text-[13px] leading-[1.6] text-[#6a6055]">
              Delete <b className="font-bold text-nexus-ink">{deleteTarget.name}</b> and its{" "}
              {deleteTarget.tasks.length} {deleteTarget.tasks.length === 1 ? "task" : "tasks"}? Copy
              task sources are left untouched.
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => setDeleteTarget(null)}>
                Cancel
              </Button>
              <Button variant="danger" onClick={() => void confirmDeleteTaskGroup()}>
                Delete group
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>

      {/* Delete task confirm */}
      <Modal
        open={!!deleteTaskTarget}
        onClose={() => setDeleteTaskTarget(null)}
        className="w-[440px]"
        overlayClassName="z-[70]"
      >
        {deleteTaskTarget ? (
          <>
            <ModalHeader
              title="Delete task"
              titleClassName="text-[16px]"
              subtitle="This removes the task and any local symlink / junction placement it created."
            />
            <div className="px-[22px] py-5 text-[13px] leading-[1.6] text-[#6a6055]">
              Delete the <b className="font-bold text-nexus-ink">{deleteTaskTarget.task.action}</b> task
              {" from "}
              <b className="font-bold text-nexus-ink">{deleteTaskTarget.group.name}</b>?
              {" Copy task sources are left untouched."}
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => setDeleteTaskTarget(null)}>
                Cancel
              </Button>
              <Button variant="danger" onClick={() => void confirmDeleteTask()}>
                Delete task
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>
    </>
  );
}

interface AddTaskFormProps {
  task: FormTask;
  onChange: (task: FormTask) => void;
  supportsJunction: boolean;
}

function AddTaskForm({ task, onChange, supportsJunction }: AddTaskFormProps) {
  const isCron = isCronSchedule(task.schedule);
  return (
    <div className="rounded-[14px] border border-nexus-border bg-nexus-sand2 p-3.5">
      <div className="mb-[11px] flex items-center justify-between gap-2.5">
        <span className="text-[11px] font-bold text-nexus-accent">New task</span>
      </div>
      <div className="mb-[11px] flex flex-wrap gap-4">
        <div>
          <div className="mb-[5px] text-[11px] font-semibold text-[#8a7d6c]">Action</div>
          <Segmented<TaskAction>
            className="bg-[#ece2d5]"
            size="sm"
            options={actionOptions(
              { sourceType: task.sourceType, targets: task.targets },
              supportsJunction,
            )}
            value={task.action}
            onChange={(a) => onChange({ ...task, action: a, schedule: scheduleForAction(a, task.schedule) })}
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
            value={task.sourceType}
            onChange={(v) => onChange({ ...task, sourceType: v })}
          />
          <Input
            className="flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
            placeholder={task.sourceType === "Cloud" ? "config/warp/" : "~/.config/warp/"}
            value={task.source}
            onChange={(e) => onChange({ ...task, source: e.target.value })}
          />
        </div>
      </div>
      <div className="mb-2.5">
        <div className="mb-[5px] flex items-center justify-between">
          <div className="text-[11px] font-semibold text-[#8a7d6c]">
            Targets <span className="font-medium text-[#b3a999]">(one or more)</span>
          </div>
          <div
            onClick={() => onChange({ ...task, targets: [...task.targets, { type: "Local", path: "" }] })}
            className="cursor-pointer text-[11.5px] font-bold text-nexus-accent hover:underline"
          >
            + Add
          </div>
        </div>
        <div className="flex flex-col gap-[7px]">
          {task.targets.map((val, j) => (
            <div key={j} className="flex items-center gap-[7px]">
              <Segmented<LocationType>
                className="bg-[#ece2d5]"
                size="sm"
                options={[{ value: "Local", label: "Local" }, { value: "Cloud", label: "Cloud" }]}
                value={val.type}
                onChange={(v) => {
                  const tg = [...task.targets];
                  tg[j] = { ...tg[j], type: v };
                  onChange({ ...task, targets: tg });
                }}
              />
              <Input
                className="flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
                placeholder={val.type === "Cloud" ? "backups/ssh/" : "/target/path/"}
                value={val.path}
                onChange={(e) => {
                  const tg = [...task.targets];
                  tg[j] = { ...tg[j], path: e.target.value };
                  onChange({ ...task, targets: tg });
                }}
              />
              <div
                onClick={() => {
                  const tg = task.targets.filter((_, k) => k !== j);
                  onChange({ ...task, targets: tg.length ? tg : [{ type: "Local", path: "" }] });
                }}
                className="inline-flex h-[30px] w-[30px] flex-none cursor-pointer items-center justify-center rounded-[8px] border border-nexus-border2 bg-white text-[15px] text-[#b3a999] hover:bg-nexus-bg hover:text-nexus-crit"
              >
                −
              </div>
            </div>
          ))}
        </div>
      </div>
      {task.action === "Copy" ? (
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
              onChange={(v) => onChange({ ...task, schedule: scheduleForMode(v, task.schedule, DEFAULT_CRON_SCHEDULE) })}
            />
            {isCron ? (
              <Input
                className="min-w-[120px] flex-1 rounded-[9px] px-[11px] py-2 font-mono text-[12px]"
                placeholder="0 5 * * *"
                value={task.schedule}
                onChange={(e) => onChange({ ...task, schedule: e.target.value || " " })}
              />
            ) : null}
          </div>
          {isCron ? (
            <div className="mt-[7px] flex flex-wrap gap-1.5">
              {SCHEDULE_PRESETS.map((cp) => (
                <Chip key={cp.expr} mono active={task.schedule === cp.expr} onClick={() => onChange({ ...task, schedule: cp.expr })}>
                  {cp.expr}
                </Chip>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
