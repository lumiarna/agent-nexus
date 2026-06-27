import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { cn } from "@/lib/utils";
import type { useGitBaseFolders } from "./useGitBaseFolders";
import type { useProjectScan } from "./useProjectScan";

interface GitBaseFoldersModalProps {
  open: boolean;
  onClose: () => void;
  baseFolders: ReturnType<typeof useGitBaseFolders>;
  scan: ReturnType<typeof useProjectScan>;
}

/** The Git Base Folders modal: register/remove discovery directories and scan
 *  them for repositories to record. State lives in the two injected hooks. */
export function GitBaseFoldersModal({
  open,
  onClose,
  baseFolders,
  scan,
}: GitBaseFoldersModalProps) {
  const { summary } = scan;
  return (
    <Modal open={open} onClose={onClose} className="max-h-[88vh] w-[560px]">
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
              value={baseFolders.path}
              onChange={(event) => baseFolders.setPath(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !baseFolders.isAdding) {
                  void baseFolders.add();
                }
              }}
              placeholder="/Users/lumiarna/Workspace"
              className="min-w-0 flex-1 rounded-[10px] border border-nexus-border2 bg-nexus-sand px-3 py-[9px] font-mono text-[12px] text-[#6a6055] outline-none focus:border-nexus-accent"
            />
            <Button
              variant="secondary"
              size="sm"
              className="rounded-[10px]"
              onClick={() => void baseFolders.add()}
              disabled={baseFolders.isAdding || !baseFolders.path.trim()}
            >
              {baseFolders.isAdding ? "Adding..." : "Add folder"}
            </Button>
          </div>
          <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
            {baseFolders.isLoading ? (
              <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                Loading base folders...
              </div>
            ) : null}

            {baseFolders.error ? (
              <div className="px-3.5 py-[11px] text-[12px] text-nexus-crit">
                {baseFolders.error}
              </div>
            ) : null}

            {!baseFolders.isLoading && !baseFolders.error && baseFolders.folders.length === 0 ? (
              <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                No base folders registered.
              </div>
            ) : null}

            {baseFolders.folders.map((bf, i) => (
              <div
                key={bf.id}
                className={cn(
                  "flex items-center justify-between gap-3 px-3.5 py-[11px]",
                  (i > 0 || baseFolders.isLoading || baseFolders.error) &&
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
                  onClick={() => void baseFolders.remove(bf.id, bf.path)}
                  disabled={baseFolders.isRemoving}
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
              onClick={() => void scan.run()}
              disabled={scan.isScanning || baseFolders.folders.length === 0}
            >
              {scan.isScanning ? "Scanning..." : "Scan all folders"}
            </Button>
          </div>
          {scan.scanError ? (
            <div className="mt-2 text-[11.5px] text-nexus-crit">{scan.scanError}</div>
          ) : null}
        </div>

        {scan.hasScanned ? (
          <div>
            <div className="mb-2 text-[12px] font-bold text-[#6a6055]">
              Discovered repositories{" "}
              <span className="font-medium text-[#b3a999]">
                {"·"} {summary.found} found {"·"} {summary.newCount} new {"·"}{" "}
                {summary.recordedCount} already recorded
              </span>
            </div>
            <div className="flex flex-col gap-0.5 overflow-hidden rounded-[12px] border border-nexus-border">
              {scan.results.length === 0 ? (
                <div className="px-3.5 py-[11px] text-[12px] text-[#b3a999]">
                  No Git repositories found in registered base folders.
                </div>
              ) : null}

              {scan.results.map((r, i) => {
                const isNew = r.state === "new";
                const on = isNew && !!scan.selection[r.path];
                return (
                  <div
                    key={r.path}
                    onClick={() => isNew && scan.toggle(r.path)}
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
        <Button variant="subtle" onClick={onClose}>
          Close
        </Button>
        {scan.hasScanned && summary.selCount > 0 && (
          <Button
            variant="primary"
            onClick={() => void scan.confirmSelected()}
            disabled={scan.isRecording}
          >
            {scan.isRecording
              ? "Recording..."
              : `Record ${summary.selCount} ${summary.selCount === 1 ? "project" : "projects"}`}
          </Button>
        )}
      </ModalFooter>
    </Modal>
  );
}
