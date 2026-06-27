import test from "node:test";
import assert from "node:assert/strict";

import type { ScanResult } from "../src/types/index.js";
import {
  canConfirmDelete,
  selectAllNew,
  selectedScanPaths,
  summarizeScan,
  toggleScanSelection,
} from "../src/components/project/projectInteractions.js";

const results: ScanResult[] = [
  { path: "/a", key: "a", state: "new" },
  { path: "/b", key: "b", state: "recorded" },
  { path: "/c", key: "c", state: "new" },
];

test("selectAllNew picks only the new repos", () => {
  assert.deepEqual(selectAllNew(results), { "/a": true, "/c": true });
});

test("toggleScanSelection flips one path without mutating the input", () => {
  const sel = { "/a": true };
  assert.deepEqual(toggleScanSelection(sel, "/c"), { "/a": true, "/c": true });
  assert.deepEqual(toggleScanSelection(sel, "/a"), { "/a": false });
  assert.deepEqual(sel, { "/a": true });
});

test("selectedScanPaths ignores recorded repos and unselected new ones", () => {
  const sel = { "/a": true, "/b": true, "/c": false };
  assert.deepEqual(selectedScanPaths(results, sel), ["/a"]);
});

test("summarizeScan reports found / new / recorded / selected counts", () => {
  assert.deepEqual(summarizeScan(results, { "/a": true }), {
    found: 3,
    newCount: 2,
    recordedCount: 1,
    selCount: 1,
  });
});

test("canConfirmDelete requires acknowledgement and no pending delete", () => {
  assert.equal(canConfirmDelete(true, false), true);
  assert.equal(canConfirmDelete(false, false), false);
  assert.equal(canConfirmDelete(true, true), false);
});
