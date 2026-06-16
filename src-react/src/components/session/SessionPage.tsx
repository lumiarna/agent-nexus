import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dot, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { useNav } from "@/lib/nav";
import { nexus } from "@/lib/mock";
import type { SessionSource } from "@/types";

type Source = Extract<SessionSource, "local" | "cloud">;

const PROJ_COLORS: Record<string, string> = {
  "agent-nexus": "#9d7a64",
  "oll-context": "#4f7a6a",
  tap: "#c2410c",
  "tap-kit": "#7a5c9e",
  "awesome-vibe-coding": "#b07d2e",
};
const ACCENT = "#9d7a64";

export function SessionPage() {
  const { go } = useNav();
  const [sessions] = useState(() => nexus.sessions());
  const [projects] = useState(() => nexus.projects().filter((p) => p.status === "active"));
  const [source, setSource] = useState<Source>("cloud");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>("s1");
  const [cloudAvailable, setCloudAvailable] = useState(true);
  const [projectId, setProjectId] = useState<string | null>(null);

  const cloudDown = source === "cloud" && !cloudAvailable;
  const q = search.trim().toLowerCase();
  let sess = sessions.filter((se) => se.source === source || se.source === "both");
  if (projectId) sess = sess.filter((se) => se.project === projectId);
  if (q)
    sess = sess.filter(
      (se) =>
        se.title.toLowerCase().includes(q) ||
        se.excerpt.toLowerCase().includes(q) ||
        se.body.toLowerCase().includes(q) ||
        se.project.toLowerCase().includes(q),
    );

  let selId = selectedId;
  if (!sess.find((se) => se.id === selId)) selId = sess.length ? sess[0].id : null;
  const sel = sess.find((se) => se.id === selId) ?? null;

  const listShown = !cloudDown && sess.length > 0;

  let emptyTitle = "";
  let emptyBody = "";
  if (cloudDown) {
    emptyTitle = "Cloud unavailable";
    emptyBody = "Could not reach the Cloud archive.";
  } else if (q && sess.length === 0) {
    emptyTitle = "No results";
    emptyBody = `No ${source} session matches “${search}”.`;
  } else if (sess.length === 0) {
    emptyTitle = source === "cloud" ? "Cloud empty" : "Local empty";
    emptyBody =
      source === "cloud"
        ? "No archived sessions in the Cloud yet."
        : "No local sessions across recorded projects.";
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex-none px-8 pb-3.5 pt-[22px]">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
              Session
            </h1>
            <p className="mt-1.5 text-[13px] text-[#9a8f80]">
              Searchable content domain · choose a source, then search within it
            </p>
          </div>
          <Segmented<Source>
            options={[
              { value: "local", label: "Local" },
              { value: "cloud", label: "Cloud" },
            ]}
            value={source}
            onChange={(v) => {
              setSource(v);
              setSelectedId(null);
            }}
          />
        </div>
        <div className="mt-3.5 flex items-center gap-3">
          <Input
            className="flex-1 rounded-full bg-nexus-card px-3.5 py-2.5 text-[13px]"
            placeholder={`Search ${source === "cloud" ? "Cloud" : "Local"} sessions by name or content`}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <span className="whitespace-nowrap text-[11.5px] text-[#b3a999]">
            {source === "cloud" ? "Cloud · aggregated (read-only)" : "Local · this device"}
          </span>
        </div>
        <div className="mt-2.5 flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-[#b3a999]">Project</span>
          <Chip
            active={projectId === null}
            onClick={() => {
              setProjectId(null);
              setSelectedId(null);
            }}
          >
            All
          </Chip>
          {projects.map((p) => (
            <Chip
              key={p.id}
              active={projectId === p.id}
              onClick={() => {
                setProjectId(p.id);
                setSelectedId(null);
              }}
            >
              {p.name}
            </Chip>
          ))}
        </div>
      </div>

      <div
        className="grid min-h-0 flex-1 border-t border-nexus-border2"
        style={{ gridTemplateColumns: "380px 1fr" }}
      >
        <div className="overflow-auto border-r border-nexus-border2 bg-nexus-sand">
          {listShown ? (
            sess.map((se) => {
              const pc = PROJ_COLORS[se.project] ?? "#a99a89";
              const selected = se.id === selId;
              return (
                <div
                  key={se.id}
                  onClick={() => setSelectedId(se.id)}
                  className="cursor-pointer border-b border-nexus-panel px-4 py-3.5"
                  style={{
                    background: selected ? "#fcf9f4" : "transparent",
                    borderLeft: `3px solid ${selected ? ACCENT : "transparent"}`,
                  }}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12.5px] font-bold text-nexus-body">
                      {se.title}
                    </div>
                    <span
                      className="flex-none rounded-[5px] px-1.5 py-px text-[10px] font-bold"
                      style={{ color: pc, background: pc + "22" }}
                    >
                      {se.project}
                    </span>
                  </div>
                  <div className="mt-[5px] line-clamp-2 text-[11.5px] leading-[1.45] text-[#9a8f80]">
                    {se.excerpt}
                  </div>
                  <div className="mt-1.5 text-[10.5px] text-[#c3b9a8]">{se.updated}</div>
                </div>
              );
            })
          ) : (
            <div className="px-[26px] py-[50px] text-center">
              <div className="text-[14px] font-bold text-[#7a6f60]">{emptyTitle}</div>
              <div className="mt-1.5 text-[12.5px] leading-[1.5] text-[#b3a999]">{emptyBody}</div>
              {cloudDown ? (
                <Button
                  variant="subtle"
                  size="sm"
                  className="mt-3.5"
                  onClick={() => {
                    setCloudAvailable(true);
                    toast("Reconnected to Cloud");
                  }}
                >
                  Retry connection
                </Button>
              ) : null}
            </div>
          )}
        </div>

        <div className="overflow-auto bg-nexus-card">
          {sel && !cloudDown ? (
            <div className="px-7 py-6">
              <div className="font-mono text-[18px] font-extrabold tracking-[-.01em] text-nexus-ink">
                {sel.title}
              </div>
              <div className="mt-2.5 flex flex-wrap gap-x-5 gap-y-1.5 text-[11.5px]">
                <span>
                  <span className="text-[#b3a999]">Project </span>
                  <span className="font-bold text-[#6a6055]">{sel.project}</span>
                </span>
                <span>
                  <span className="text-[#b3a999]">Updated </span>
                  <span className="text-[#6a6055]">{sel.updated}</span>
                </span>
                <span>
                  <span className="text-[#b3a999]">Size </span>
                  <span className="text-[#6a6055]">{sel.size}</span>
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span className="text-[#b3a999]">Source </span>
                  <span className="inline-flex items-center gap-1.5 font-bold text-[#6a6055]">
                    <Dot color={source === "cloud" ? ACCENT : "#8a9a5b"} />
                    {source === "cloud" ? "Cloud" : "Local"}
                  </span>
                </span>
              </div>
              <div className="mt-1.5 font-mono text-[11px] text-[#c3b9a8]">{sel.file}</div>

              <div className="mt-4 flex flex-wrap items-center gap-2">
                <Button
                  variant="primary"
                  size="sm"
                  className="px-3.5"
                  onClick={() => toast(`Open file · ${sel.file}`)}
                >
                  Open file
                </Button>
                <Button
                  variant="subtle"
                  size="sm"
                  className="px-3.5"
                  onClick={() => go("project", { projectId: sel.project })}
                >
                  Open Project ↗
                </Button>
                <div className="mx-1 h-[22px] w-px bg-nexus-border" />
                <Button
                  variant="subtle"
                  size="sm"
                  className="px-3.5"
                  onClick={() => toast(`Archive now → ${sel.project} (Project-level, one-way)`)}
                >
                  Archive now
                </Button>
                <Button
                  variant="subtle"
                  size="sm"
                  className="px-3.5"
                  onClick={() => toast(`Pull now → ${sel.project} (Project-level, one-way)`)}
                >
                  Pull now
                </Button>
              </div>
              <div className="mt-2 text-[11px] text-[#c3b9a8]">
                Open Project jumps to this session's Project detail. Quick actions run at Project
                granularity; Archive and Pull are separate one-way tasks.
              </div>

              <div className="mt-[18px] whitespace-pre-wrap rounded-[14px] border border-nexus-panel bg-[#f8f3ea] px-[22px] py-5 font-mono text-[12px] leading-[1.7] text-[#4a4138]">
                {sel.body}
              </div>
            </div>
          ) : (
            <div className="flex h-full items-center justify-center text-[13px] text-[#c3b9a8]">
              Select a session to preview
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
