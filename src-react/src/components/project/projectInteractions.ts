/**
 * Pure decision logic for the Project list-screen interactions (scan selection
 * and delete confirmation). Kept free of React/react-query/Tauri so the rules
 * can be unit-tested without rendering the page.
 */
import type { ScanResult } from "@/types/index.js";

/** A path → selected map for the scan results. */
export type ScanSelection = Record<string, boolean>;

/** Select every newly-discovered repo (recorded ones are never selectable). */
export function selectAllNew(results: ScanResult[]): ScanSelection {
  const selection: ScanSelection = {};
  for (const repo of results) {
    if (repo.state === "new") selection[repo.path] = true;
  }
  return selection;
}

/** Flip the selection for one path (new array, original untouched). */
export function toggleScanSelection(selection: ScanSelection, path: string): ScanSelection {
  return { ...selection, [path]: !selection[path] };
}

/** The new repo paths the user has selected, in scan-result order. */
export function selectedScanPaths(results: ScanResult[], selection: ScanSelection): string[] {
  return results
    .filter((repo) => repo.state === "new" && selection[repo.path])
    .map((repo) => repo.path);
}

export interface ScanSummary {
  found: number;
  newCount: number;
  recordedCount: number;
  selCount: number;
}

/** Counts shown in the scan section header and used to gate the record button. */
export function summarizeScan(results: ScanResult[], selection: ScanSelection): ScanSummary {
  const newCount = results.filter((repo) => repo.state === "new").length;
  const selCount = selectedScanPaths(results, selection).length;
  return {
    found: results.length,
    newCount,
    recordedCount: results.length - newCount,
    selCount,
  };
}

/** The delete button only fires once the user has acknowledged and no delete is in flight. */
export function canConfirmDelete(acknowledged: boolean, pending: boolean): boolean {
  return acknowledged && !pending;
}
