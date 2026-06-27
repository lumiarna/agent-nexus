import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  useRecordProjectsMutation,
  useScanGitBaseFoldersMutation,
} from "@/lib/query/projects";
import { getErrorMessage } from "./getErrorMessage";
import {
  selectAllNew,
  selectedScanPaths,
  summarizeScan,
  toggleScanSelection,
  type ScanSummary,
} from "./projectInteractions";

/**
 * Project discovery cluster: run a scan over the registered Git base folders,
 * track which newly-found repos are selected, and record the selection. The
 * `hasScanned` / `selection` state and both mutations stay hidden behind this
 * small interface.
 */
export function useProjectScan(baseFolderCount: number, onRecorded: () => void) {
  const scan = useScanGitBaseFoldersMutation();
  const recordProjects = useRecordProjectsMutation();
  const [hasScanned, setHasScanned] = useState(false);
  const [selection, setSelection] = useState<Record<string, boolean>>({});

  const results = hasScanned ? scan.data ?? [] : [];
  const summary: ScanSummary = summarizeScan(results, selection);

  function reset() {
    setHasScanned(false);
    setSelection({});
    scan.reset();
  }

  // A changed base-folder set invalidates any prior scan results.
  useEffect(() => {
    setHasScanned(false);
    setSelection({});
    scan.reset();
    // Only the folder count should retrigger this; `scan` is stable enough.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [baseFolderCount]);

  function toggle(path: string) {
    setSelection((current) => toggleScanSelection(current, path));
  }

  async function run() {
    if (baseFolderCount === 0) {
      toast("Add a Git base folder before scanning");
      return;
    }

    try {
      const scanned = await scan.mutateAsync();
      setHasScanned(true);
      setSelection(selectAllNew(scanned));
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function confirmSelected() {
    const paths = selectedScanPaths(results, selection);
    if (paths.length === 0) return;

    try {
      await recordProjects.mutateAsync(paths);
      reset();
      toast(`Recorded ${paths.length} ${paths.length === 1 ? "project" : "projects"}`);
      onRecorded();
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return {
    hasScanned,
    results,
    summary,
    selection,
    toggle,
    run,
    confirmSelected,
    reset,
    isScanning: scan.isPending,
    isRecording: recordProjects.isPending,
    scanError: scan.error ? getErrorMessage(scan.error) : null,
  };
}
