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
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/primitives";
import { ScreenScroll } from "@/components/shell/screen";
import { useReorderProjectsMutation } from "@/lib/query/projects";
import { cn } from "@/lib/utils";
import type { Project } from "@/types";
import { getErrorMessage } from "./getErrorMessage";
import { DEFAULT_SESSIONS_DIR } from "./projectShared";

// drag | key+path | skill | prompt | session | ⋯
const LIST_COLS = "20px 1.6fr 1fr 1fr 1fr 36px";

interface MenuState {
  id: string;
  y: number;
  right: number;
}

function moveProjectOrder(order: string[], fromId: string, toId: string): string[] | null {
  if (!fromId || fromId === toId) return null;
  const fromIndex = order.indexOf(fromId);
  const toIndex = order.indexOf(toId);
  if (fromIndex < 0 || toIndex < 0) return null;
  return arrayMove(order, fromIndex, toIndex);
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

interface ProjectListViewProps {
  projects: Project[];
  isLoading: boolean;
  error: string | null;
  isRefreshing: boolean;
  onRefresh: () => void;
  onOpenBaseFolders: () => void;
  onOpenAddProject: () => void;
  onOpenDetail: (id: string) => void;
  onRequestDelete: (id: string) => void;
}

/**
 * Presentational list screen: drag-to-reorder, the overflow menu (relocate /
 * hide / delete), and the hidden-projects tray. Owns its own ordering, hidden
 * and menu state plus the reorder mutation; navigation and deletion are raised
 * to the parent through callbacks.
 */
export function ProjectListView({
  projects,
  isLoading,
  error,
  isRefreshing,
  onRefresh,
  onOpenBaseFolders,
  onOpenAddProject,
  onOpenDetail,
  onRequestDelete,
}: ProjectListViewProps) {
  const reorderProjects = useReorderProjectsMutation();
  const [order, setOrder] = useState<string[]>([]);
  const [hiddenIds, setHiddenIds] = useState<string[]>([]);
  const [menu, setMenu] = useState<MenuState | null>(null);

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

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  function openMenu(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({ id, y: r.bottom + 4, right: Math.max(16, window.innerWidth - r.right) });
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
    } catch (err) {
      setOrder(previousOrder);
      toast(getErrorMessage(err));
    }
  }

  const menuItem = "cursor-pointer rounded-[8px] px-3.5 py-[9px] text-[12.5px] font-semibold";

  return (
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
          <Button variant="subtle" size="sm" onClick={onRefresh} disabled={isRefreshing}>
            <RefreshCw size={14} className={cn(isRefreshing && "animate-spin")} />
            {isRefreshing ? "Refreshing..." : "Refresh"}
          </Button>
          <Button variant="secondary" onClick={onOpenBaseFolders}>
            Git Base Folders
          </Button>
          <Button variant="primary" onClick={onOpenAddProject}>
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

        {isLoading ? (
          <div className="px-5 py-8 text-center text-[12.5px] text-[#b3a999]">
            Loading projects...
          </div>
        ) : null}

        {error ? (
          <div className="mx-5 my-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] px-4 py-3 text-[12.5px] text-nexus-crit">
            {error}
          </div>
        ) : null}

        {!isLoading && !error && ordered.length === 0 ? (
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
                  onClick={isStale ? undefined : () => onOpenDetail(p.id)}
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
              onClick={() => { const id = menu.id; setMenu(null); onRequestDelete(id); }}
              className={cn(menuItem, "text-nexus-crit hover:bg-[#f8ebe6]")}
            >
              Delete…
            </div>
          </div>
        </>
      ) : null}
    </ScreenScroll>
  );
}
