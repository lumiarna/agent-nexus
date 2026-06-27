import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { ScreenScroll } from "@/components/shell/screen";
import { Card } from "@/components/ui/primitives";
import { isTauriRuntime } from "@/lib/runtime";
import { useProjectsQuery } from "@/lib/query/projects";
import { getErrorMessage } from "./getErrorMessage";
import { DeleteProjectModal } from "./DeleteProjectModal";
import { GitBaseFoldersModal } from "./GitBaseFoldersModal";
import { ProjectDetailView } from "./ProjectDetailView";
import { ProjectListView } from "./ProjectListView";
import { useAddProject } from "./useAddProject";
import { useGitBaseFolders } from "./useGitBaseFolders";
import { useProjectDeletion } from "./useProjectDeletion";
import { useProjectScan } from "./useProjectScan";

function deriveProjectKey(path: string): string {
  const parts = path
    .trim()
    .replace(/[\\/]+$/, "")
    .split(/[\\/]/)
    .filter(Boolean);
  return parts[parts.length - 1] ?? "";
}

/**
 * Thin assembly shell: routes between the list and detail views and owns the
 * three list-screen overlays. Every feature cluster (scan / add / base folders /
 * deletion) lives in its own hook; the views are presentational.
 */
export function ProjectPage({ initialProjectId }: { initialProjectId?: string }) {
  const desktop = isTauriRuntime();
  const projectsQuery = useProjectsQuery();
  const projects = projectsQuery.data ?? [];
  const projectError = projectsQuery.error ? getErrorMessage(projectsQuery.error) : null;

  const [screen, setScreen] = useState<"list" | "detail">(
    initialProjectId ? "detail" : "list",
  );
  const [detailId, setDetailId] = useState(initialProjectId ?? "");

  const baseFolders = useGitBaseFolders();
  const scan = useProjectScan(baseFolders.folders.length, () => baseFolders.setOpen(false));
  const add = useAddProject((project) => {
    setDetailId(project.id);
    setScreen("detail");
  });
  const deletion = useProjectDeletion(projects);

  const dp = projects.find((p) => p.id === detailId) ?? projects[0];
  const addKey = deriveProjectKey(add.path);
  const isRefreshing = projectsQuery.isFetching || baseFolders.isFetching;

  async function handleRefresh() {
    if (!desktop) {
      toast("Desktop runtime required for refreshing");
      return;
    }
    try {
      const [projectsResult] = await Promise.all([projectsQuery.refetch(), baseFolders.refetch()]);
      const count = projectsResult.data?.length ?? 0;
      toast(`Refreshed ${count} ${count === 1 ? "project" : "projects"}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return (
    <>
      {screen === "list" ? (
        <ProjectListView
          projects={projects}
          isLoading={projectsQuery.isLoading}
          error={projectError}
          isRefreshing={isRefreshing}
          onRefresh={() => void handleRefresh()}
          onOpenBaseFolders={() => {
            scan.reset();
            baseFolders.setOpen(true);
          }}
          onOpenAddProject={() => add.setOpen(true)}
          onOpenDetail={(id) => {
            setDetailId(id);
            setScreen("detail");
          }}
          onRequestDelete={(id) => deletion.request(id)}
        />
      ) : dp ? (
        <ProjectDetailView project={dp} desktop={desktop} onBack={() => setScreen("list")} />
      ) : (
        <ScreenScroll>
          <button
            onClick={() => setScreen("list")}
            className="mb-3.5 inline-flex items-center gap-1.5 text-[12px] text-[#9a8f80] hover:text-nexus-accent"
          >
            ← Project
          </button>
          <Card className="p-[22px] text-[12.5px] text-[#9a8f80]">No project is selected.</Card>
        </ScreenScroll>
      )}

      {/* Add Project modal */}
      <Modal open={add.open} onClose={() => add.setOpen(false)} className="w-[480px]">
        <ModalHeader title="Add Project" subtitle="Record a single Git repository root" />
        <div className="flex flex-col gap-4 px-[22px] py-5">
          <div>
            <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">Repository root</div>
            <input
              value={add.path}
              onChange={(event) => add.setPath(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !add.isPending) void add.submit();
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
          <Button variant="subtle" onClick={() => add.setOpen(false)}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={() => void add.submit()}
            disabled={add.isPending || !add.path.trim()}
          >
            {add.isPending ? "Recording..." : "Record project"}
          </Button>
        </ModalFooter>
      </Modal>

      <GitBaseFoldersModal
        open={baseFolders.open}
        onClose={() => baseFolders.setOpen(false)}
        baseFolders={baseFolders}
        scan={scan}
      />

      <DeleteProjectModal deletion={deletion} />
    </>
  );
}
