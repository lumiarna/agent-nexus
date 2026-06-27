import test from "node:test";
import assert from "node:assert/strict";

import { resolveAdd, resolveRemove } from "../src/components/project/stringListEdit.js";

test("resolveAdd trims surrounding whitespace before accepting", () => {
  const result = resolveAdd("  skills  ", []);
  assert.deepEqual(result, { kind: "ok", value: "skills", next: ["skills"] });
});

test("resolveAdd treats blank input as a no-op", () => {
  assert.deepEqual(resolveAdd("   ", ["a"]), { kind: "empty" });
});

test("resolveAdd dedups against existing items (after trim)", () => {
  assert.deepEqual(resolveAdd(" a ", ["a", "b"]), { kind: "duplicate" });
});

test("resolveAdd surfaces the validate error and rejects", () => {
  const validate = (value: string) => (value.endsWith(".md") ? null : "must end with .md");
  assert.deepEqual(resolveAdd("notes.txt", [], validate), {
    kind: "invalid",
    message: "must end with .md",
  });
  assert.deepEqual(resolveAdd("CLAUDE.md", [], validate), {
    kind: "ok",
    value: "CLAUDE.md",
    next: ["CLAUDE.md"],
  });
});

test("resolveAdd does not mutate the input list", () => {
  const items = ["a"];
  const result = resolveAdd("b", items);
  assert.deepEqual(items, ["a"]);
  assert.deepEqual(result, { kind: "ok", value: "b", next: ["a", "b"] });
});

test("resolveRemove drops the value without mutating the input", () => {
  const items = ["a", "b", "c"];
  assert.deepEqual(resolveRemove("b", items), ["a", "c"]);
  assert.deepEqual(items, ["a", "b", "c"]);
});
