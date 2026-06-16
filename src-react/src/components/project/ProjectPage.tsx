import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, Dot } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { ScreenScroll } from "@/components/shell/screen";
import { useNav } from "@/lib/nav";
import { nexus } from "@/lib/mock";
import { palette, toggleCellRole } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { AgentName, GitBaseFolder, Project } from "@/types";

const LIST_COLS = "20px 1.5fr 1.8fr 220px 36px";
type DetailSource = "local" | "cloud";

interface MenuState {
  id: string;
  y: number;
  right: number;
}

export function ProjectPage({ initialProjectId }: { initialProjectId?: string }) {
  const { go } = useNav();
  const [projects] = useState(() => nexus.projects());
  const [order, setOrder] = useState<string[]>(() => projects.map((p) => p.id));
  const [dragId, setDragId] = useState<string | null>(null);
  const [skills, setSkills] = useState(() => nexus.skills());
  const [screen, setScreen] = useState<"list" | "detail">(
    initialProjectId ? "detail" : "list",
  );
  const [detailId, setDetailId] = useState(initialProjectId ?? "oll-context");
  const [detailSource, setDetailSource] = useState<DetailSource>("local");
  const [hiddenIds, setHiddenIds] = useState<string[]>([]);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [baseFoldersOpen, setBaseFoldersOpen] = useState(false);
  const [baseFolders, setBaseFolders] = useState<GitBaseFolder[]>(() => nexus.gitBaseFolders());
  const [addOpen, setAddOpen] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [scanSel, setScanSel] = useState<Record<string, boolean>>({});
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [deleteAck, setDeleteAck] = useState(false);

  const hidden = (id: string) => hiddenIds.includes(id);
  const byId = Object.fromEntries(projects.map((p) => [p.id, p]));
  const ordered = order.map((id) => byId[id]).filter(Boolean) as Project[];
  const active = ordered.filter((p) => p.status === "active" && !hidden(p.id));
  const stale = ordered.filter((p) => p.status === "stale" && !hidden(p.id));
  const hiddenP = ordered.filter((p) => p.status === "hidden" || hidden(p.id));

  const menuProject = menu ? projects.find((p) => p.id === menu.id) : null;
  const del = deleteId ? projects.find((p) => p.id === deleteId) ?? null : null;

  function openMenu(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({ id, y: r.bottom + 4, right: Math.max(16, window.innerWidth - r.right) });
  }

  function reorder(fromId: string | null, toId: string) {
    if (!fromId || fromId === toId) return;
    setOrder((o) => {
      const a = [...o];
      const fi = a.indexOf(fromId);
      const ti = a.indexOf(toId);
      if (fi < 0 || ti < 0) return o;
      a.splice(fi, 1);
      a.splice(ti, 0, fromId);
      return a;
    });
  }
  // Native HTML5 drag-reorder, shared by active & stale rows (Display Order).
  function dragProps(id: string) {
    return {
      draggable: true,
      onDragStart: (e: React.DragEvent<HTMLDivElement>) => {
        setDragId(id);
        e.dataTransfer.effectAllowed = "move";
        try { e.dataTransfer.setData("text/plain", id); } catch { /* noop */ }
      },
      onDragOver: (e: React.DragEvent<HTMLDivElement>) => {
        if (dragId) { e.preventDefault(); e.dataTransfer.dropEffect = "move"; }
      },
      onDrop: (e: React.DragEvent<HTMLDivElement>) => {
        if (dragId) { e.preventDefault(); reorder(dragId, id); setDragId(null); }
      },
      onDragEnd: () => setDragId(null),
    };
  }

  // Scan modal derived state
  const scanRes = scanned ? nexus.scanResults() : [];
  const newCount = scanRes.filter((r) => r.state === "new").length;
  const selCount = scanRes.filter((r) => r.state === "new" && scanSel[r.path]).length;

  // Detail derived state
  const dp = projects.find((p) => p.id === detailId) ?? projects[0];
  const dpSkills = skills.filter((k) => k.scope === "project" && k.projectId === dp.id);
  const dpSessions = nexus.sessionsForProject(dp.id, detailSource);

  const toggleCell = (id: string, agent: AgentName) =>
    setSkills((s) =>
      s.map((k) => (k.id === id ? { ...k, cells: toggleCellRole(k.cells, agent) } : k)),
    );
  const toggleDmi = (id: string) =>
    setSkills((s) => s.map((k) => (k.id === id ? { ...k, disabled: !k.disabled } : k)));

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
              <Button variant="secondary" onClick={() => { setBaseFoldersOpen(true); setScanned(false); setScanSel({}); }}>
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
              <div>Repo path</div>
              <div className="text-right">Assets</div>
              <div />
            </div>

            {active.map((p) => (
              <div
                key={p.id}
                {...dragProps(p.id)}
                onClick={() => { setDetailId(p.id); setScreen("detail"); }}
                className={cn(
                  "grid cursor-pointer items-center gap-4 border-b border-[#f3eee5] px-5 py-[13px] hover:bg-[#fbf6ef]",
                  dragId === p.id && "opacity-50",
                )}
                style={{ gridTemplateColumns: LIST_COLS }}
              >
                <div className="flex cursor-grab items-center justify-center text-[11px] tracking-[1px] text-[#d0c4b4]" title="Drag to reorder">
                  ⋮⋮
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-[14.5px] font-bold text-nexus-ink">{p.name}</span>
                    <span className="rounded-[5px] bg-[#e9eed8] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#5f7a3e]">
                      Active
                    </span>
                  </div>
                  <div className="mt-[3px] font-mono text-[11px] text-[#b3a999]">
                    {p.sessionsDir}
                    {p.sessionsNote ?? ""}
                  </div>
                </div>
                <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-[#8a8073]">
                  {p.path}
                </div>
                <div className="flex justify-end gap-[7px]">
                  {[
                    { label: "SKILL", n: p.skills },
                    { label: "SESSION", n: p.sessions },
                    { label: "SYNC", n: p.sync },
                  ].map((c) => (
                    <div
                      key={c.label}
                      className="flex min-w-[48px] flex-col items-center rounded-[10px] bg-nexus-bg py-1.5"
                    >
                      <span className="text-[14px] font-extrabold text-nexus-body">{c.n}</span>
                      <span className="text-[9px] tracking-[.03em] text-[#b3a999]">{c.label}</span>
                    </div>
                  ))}
                </div>
                <div
                  onClick={(e) => openMenu(e, p.id)}
                  className="flex h-[30px] w-[30px] cursor-pointer items-center justify-center rounded-[8px] text-[16px] tracking-[2px] text-[#a99a89] hover:bg-nexus-panel hover:text-[#7a6f60]"
                >
                  ⋯
                </div>
              </div>
            ))}

            {stale.map((p) => (
              <div
                key={p.id}
                {...dragProps(p.id)}
                className={cn(
                  "grid items-center gap-4 border-b border-[#f3eee5] bg-[#faf3e8] px-5 py-4",
                  dragId === p.id && "opacity-50",
                )}
                style={{ gridTemplateColumns: LIST_COLS }}
              >
                <div className="flex cursor-grab items-center justify-center text-[11px] tracking-[1px] text-[#d9ccb8]" title="Drag to reorder">
                  ⋮⋮
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-[14.5px] font-bold text-[#6a6055]">{p.name}</span>
                    <span className="rounded-[5px] bg-[#f7eccb] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#9a6f0a]">
                      Stale
                    </span>
                  </div>
                  <div className="mt-[3px] text-[11px] text-[#bca37a]">Repo path no longer resolves</div>
                </div>
                <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-[#bca37a] line-through">
                  {p.path}
                </div>
                <div />
                <div
                  onClick={(e) => openMenu(e, p.id)}
                  className="flex h-[30px] w-[30px] cursor-pointer items-center justify-center rounded-[8px] text-[16px] tracking-[2px] text-[#a99a89] hover:bg-nexus-panel hover:text-[#7a6f60]"
                >
                  ⋯
                </div>
              </div>
            ))}
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
      ) : (
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
              <span className="text-[11px] text-[#b3a999]">
                project scope · {dpSkills.length} {dpSkills.length === 1 ? "skill" : "skills"}
              </span>
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
            {dpSkills.length > 0 ? (
              dpSkills.map((k) => (
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
              <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
                No project skills recorded for this repository.
              </div>
            )}
            <button onClick={() => go("skill")} className="mx-5 mb-[18px] inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
              Open in Skill →
            </button>
          </Card>

          {/* Session panel */}
          <Card className="mt-4 p-5">
            <div className="mb-3.5 flex items-center justify-between">
              <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                Session
              </span>
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
            {dpSessions.length > 0 ? (
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
                  detail: `${dpSkills.length} managed relations from the Skill matrix`,
                  status: "Linked",
                  fg: "#5f7a3e",
                  dot: palette.good,
                },
                {
                  label: "Session Backup",
                  detail: `cloud://agent-nexus/${dp.key}`,
                  status: "Healthy",
                  fg: "#5f7a3e",
                  dot: palette.good,
                },
                {
                  label: "Generic File",
                  detail: dp.id === "tap" ? "TAP symlinks · 2 targets" : "No generic tasks bound",
                  status: dp.id === "tap" ? "OK" : "None",
                  fg: dp.id === "tap" ? "#5f7a3e" : "#a99a89",
                  dot: dp.id === "tap" ? palette.good : "#d9c9b3",
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
      )}

      {/* Add Project modal */}
      <Modal open={addOpen} onClose={() => setAddOpen(false)} className="w-[480px]">
        <ModalHeader title="Add Project" subtitle="Record a single Git repository root" />
        <div className="flex flex-col gap-4 px-[22px] py-5">
          <div>
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Repository root</div>
            <div className="flex items-center gap-2 rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055]">
              D:/Workspace/new-service
              <span className="ml-auto cursor-pointer font-sans text-[11px] font-semibold text-nexus-accent">
                Browse…
              </span>
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Project Key</div>
            <div className="flex items-center gap-2 rounded-[10px] border border-nexus-border2 bg-nexus-bg px-3 py-[9px] font-mono text-[12.5px] text-[#6a6055]">
              new-service
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
            onClick={() => { setAddOpen(false); toast("Project recorded \u00b7 key \u201cnew-service\u201d (folder name)"); }}
          >
            Record project
          </Button>
        </ModalFooter>
      </Modal>

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
              <button
                onClick={() => {
                  const newPath = "E:/Projects";
                  if (baseFolders.some((x) => x.path === newPath)) { toast("Already registered"); return; }
                  setBaseFolders((bf) => [...bf, { path: newPath, addedAt: new Date().toISOString().slice(0, 10) }]);
                  toast(`Added base folder \u00b7 ${newPath}`);
                }}
                className="ml-auto rounded-full border border-nexus-border2 bg-nexus-card px-2.5 py-1 text-[11px] font-semibold text-nexus-accent hover:bg-nexus-sand"
              >
                + Add folder
              </button>
            </div>
            <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
              {baseFolders.map((bf, i) => (
                <div
                  key={bf.path}
                  className={cn(
                    "flex items-center justify-between gap-3 px-3.5 py-[11px]",
                    i > 0 && "border-t border-[#f3eee5]",
                  )}
                >
                  <div className="min-w-0">
                    <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-nexus-body">
                      {bf.path}
                    </div>
                    <div className="mt-0.5 text-[10.5px] text-[#b3a999]">Added {bf.addedAt}</div>
                  </div>
                  <button
                    onClick={() => {
                      setBaseFolders((bfs) => bfs.filter((x) => x.path !== bf.path));
                      toast(`Removed base folder \u00b7 ${bf.path}`);
                    }}
                    className="flex-none text-[11px] font-semibold text-nexus-crit hover:underline"
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
                onClick={() => {
                  const sel: Record<string, boolean> = {};
                  nexus.scanResults().forEach((r) => { if (r.state === "new") sel[r.path] = true; });
                  setScanned(true);
                  setScanSel(sel);
                }}
              >
                Scan all folders
              </Button>
            </div>
          </div>

          {scanned ? (
            <div>
              <div className="mb-2 text-[12px] font-bold text-[#6a6055]">
                Discovered repositories{" "}
                <span className="font-medium text-[#b3a999]">
                  {"\u00b7"} {scanRes.length} found {"\u00b7"} {newCount} new {"\u00b7"}{" "}
                  {scanRes.length - newCount} already recorded
                </span>
              </div>
              <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
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
          {scanned && selCount > 0 && (
            <Button
              variant="primary"
              onClick={() => {
                setBaseFoldersOpen(false);
                toast(`Recorded ${selCount} ${selCount === 1 ? "project" : "projects"}`);
              }}
            >
              Record {selCount} {selCount === 1 ? "project" : "projects"}
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
                  if (!deleteAck) return;
                  const n = del.name;
                  setDeleteId(null);
                  setDeleteAck(false);
                  toast(`${n} permanently deleted`);
                }}
                className={cn(
                  "rounded-full px-[18px] py-[9px] text-[12.5px] font-bold text-white",
                  deleteAck
                    ? "cursor-pointer bg-nexus-crit shadow-[0_2px_8px_rgba(181,84,64,.32)]"
                    : "cursor-not-allowed bg-[#d9b6ab]",
                )}
              >
                Delete permanently
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
