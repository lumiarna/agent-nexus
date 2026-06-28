import { useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dot, Input } from "@/components/ui/primitives";
import { Chip, Segmented } from "@/components/ui/segmented";
import { Markdown } from "@/components/ui/markdown";
import { useNav } from "@/lib/nav";
import { useProjectsQuery } from "@/lib/query/projects";
import {
  useCloudSessionQuery,
  useCloudSessionsQuery,
  useLocalSessionQuery,
  useLocalSessionsQuery,
} from "@/lib/query/sessions";
import { isTauriRuntime } from "@/lib/runtime";
import { cn } from "@/lib/utils";
import type { Session, SessionSource } from "@/types";

type Source = Extract<SessionSource, "local" | "cloud">;

const PROJECT_COLORS = ["#9d7a64", "#4f7a6a", "#c2410c", "#7a5c9e", "#b07d2e", "#5a7894"];
const ACCENT = "#9d7a64";

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

function projectColor(key: string): string {
  let hash = 0;
  for (const char of key) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return PROJECT_COLORS[hash % PROJECT_COLORS.length];
}

export function SessionPage() {
  const { go } = useNav();
  const desktop = isTauriRuntime();
  const projectsQuery = useProjectsQuery();
  const localSessionsQuery = useLocalSessionsQuery();
  const cloudSessionsQuery = useCloudSessionsQuery();
  const [source, setSource] = useState<Source>("local");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const activeSessionsQuery = source === "local" ? localSessionsQuery : cloudSessionsQuery;
  const isRealSource = desktop;
  const sessions = activeSessionsQuery.data ?? [];
  const projects = (projectsQuery.data ?? []).filter((p) => p.status === "active");
  const queryError =
    isRealSource && activeSessionsQuery.error
      ? getErrorMessage(activeSessionsQuery.error)
      : null;
  const pageError = queryError;
  const isLoading = isRealSource && activeSessionsQuery.isLoading;
  const isRefreshing = isRealSource && activeSessionsQuery.isFetching;

  async function refreshSessions() {
    if (!desktop) {
      toast("Desktop runtime required for session refresh");
      return;
    }

    try {
      const rows = await activeSessionsQuery.refetch();
      if (rows.data) {
        toast(`Refreshed ${rows.data.length} ${rows.data.length === 1 ? "session" : "sessions"}`);
      }
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  const q = search.trim().toLowerCase();
  const projectCounts = new Map<string, number>();
  for (const session of sessions) {
    if (session.source !== source && session.source !== "both") continue;
    projectCounts.set(session.project, (projectCounts.get(session.project) ?? 0) + 1);
  }
  let sess = sessions.filter((se) => se.source === source || se.source === "both");
  if (projectId) sess = sess.filter((se) => se.project === projectId);
  if (q)
    sess = sess.filter(
      (se) =>
        se.title.toLowerCase().includes(q) ||
        se.excerpt.toLowerCase().includes(q) ||
        (!isRealSource && se.body.toLowerCase().includes(q)) ||
        (se.projectName ?? "").toLowerCase().includes(q) ||
        se.project.toLowerCase().includes(q),
    );

  let selId = selectedId;
  if (!sess.find((se) => se.id === selId)) selId = sess.length ? sess[0].id : null;
  const sel = sess.find((se) => se.id === selId) ?? null;
  const localSessionDetail = useLocalSessionQuery(
    sel?.id ?? null,
    isRealSource && source === "local" && sel != null && !pageError,
  );
  const cloudSessionDetail = useCloudSessionQuery(
    sel?.id ?? null,
    isRealSource && source === "cloud" && sel != null && !pageError,
  );
  const activeSessionDetail = source === "local" ? localSessionDetail : cloudSessionDetail;
  const projectNameById = new Map(projects.map((project) => [project.id, project.name]));
  const sessionProjectLabel = (session: Session) =>
    session.projectName ?? projectNameById.get(session.project) ?? session.project;
  const previewBody = isRealSource ? activeSessionDetail.data?.body ?? "" : sel?.body ?? "";
  const previewError =
    isRealSource && activeSessionDetail.error
      ? getErrorMessage(activeSessionDetail.error)
      : null;
  const isPreviewLoading =
    isRealSource && sel != null && activeSessionDetail.isLoading && !previewError;

  const listShown = !pageError && !isLoading && sess.length > 0;

  let emptyTitle = "";
  let emptyBody = "";
  if (isLoading && sess.length === 0) {
    emptyTitle = "Refreshing sessions";
    emptyBody =
      source === "cloud"
        ? "Reading Cloud session archive across recorded projects."
        : "Reading local session directories across recorded projects.";
  } else if (pageError) {
    emptyTitle = "Session refresh failed";
    emptyBody = pageError;
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
          <div className="flex items-center gap-2.5">
            <Button
              variant="subtle"
              size="sm"
              disabled={isRefreshing}
              onClick={() => void refreshSessions()}
            >
              <RefreshCw size={14} className={cn(isRefreshing && "animate-spin")} />
              {isRefreshing ? "Refreshing..." : "Refresh"}
            </Button>
            <Segmented<Source>
              options={[
                { value: "local", label: "Local" },
                { value: "cloud", label: "Cloud" },
              ]}
              value={source}
              onChange={(v) => {
                setSource(v);
                setProjectId(null);
                setSelectedId(null);
              }}
            />
          </div>
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
              <span>{p.name}</span>
              {projectCounts.get(p.id) ? (
                <span className="ml-1 opacity-80">{projectCounts.get(p.id)}</span>
              ) : null}
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
              const projectLabel = sessionProjectLabel(se);
              const pc = projectColor(projectLabel || se.project);
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
                      {projectLabel}
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
            </div>
          )}
        </div>

        <div className="overflow-auto bg-nexus-card">
          {sel && !pageError && !isLoading ? (
            <div className="px-7 py-6">
              <div className="font-mono text-[18px] font-extrabold tracking-[-.01em] text-nexus-ink">
                {sel.title}
              </div>
              <div className="mt-2.5 flex flex-wrap gap-x-5 gap-y-1.5 text-[11.5px]">
                <span>
                  <span className="text-[#b3a999]">Project </span>
                  <span className="font-bold text-[#6a6055]">{sessionProjectLabel(sel)}</span>
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
                  onClick={() =>
                    toast(`Archive now → ${sessionProjectLabel(sel)} (Project-level, one-way)`)
                  }
                >
                  Archive now
                </Button>
                <Button
                  variant="subtle"
                  size="sm"
                  className="px-3.5"
                  onClick={() =>
                    toast(`Pull now → ${sessionProjectLabel(sel)} (Project-level, one-way)`)
                  }
                >
                  Pull now
                </Button>
              </div>
              <div className="mt-2 text-[11px] text-[#c3b9a8]">
                Open Project jumps to this session's Project detail. Quick actions run at Project
                granularity; Archive and Pull are separate one-way tasks.
              </div>

              <div className="mt-[18px] rounded-[14px] border border-nexus-panel bg-nexus-card px-[22px] py-5">
                {previewError ? (
                  <div className="font-mono text-[12px] leading-[1.7] text-nexus-crit">
                    Session preview failed: {previewError}
                  </div>
                ) : isPreviewLoading ? (
                  <div className="font-mono text-[12px] leading-[1.7] text-[#9a8f80]">
                    Loading session preview...
                  </div>
                ) : (
                  <Markdown>{previewBody}</Markdown>
                )}
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
