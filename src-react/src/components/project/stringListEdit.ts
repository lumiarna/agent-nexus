/**
 * Pure decision logic shared by the Project custom-source editors
 * (`StringListConfigModal` and `SingleValueConfigModal`).
 *
 * It carries the trim / validate / dedup rules so the React shells stay
 * presentational and the rules can be unit-tested without react-query or Tauri.
 */

/** Outcome of trying to add a value to a string list. */
export type AddOutcome =
  /** Input was blank after trimming — caller should do nothing. */
  | { kind: "empty" }
  /** `validate` rejected the value; `message` is the reason to surface. */
  | { kind: "invalid"; message: string }
  /** Value already present in the list. */
  | { kind: "duplicate" }
  /** Accepted; `value` is the trimmed entry, `next` the resulting list. */
  | { kind: "ok"; value: string; next: string[] };

/**
 * Resolve an add request: trim, then run the optional `validate`, then dedup
 * against `items`. Never mutates `items`.
 */
export function resolveAdd(
  input: string,
  items: string[],
  validate?: (value: string) => string | null,
): AddOutcome {
  const value = input.trim();
  if (!value) return { kind: "empty" };
  const invalid = validate?.(value);
  if (invalid) return { kind: "invalid", message: invalid };
  if (items.includes(value)) return { kind: "duplicate" };
  return { kind: "ok", value, next: [...items, value] };
}

/** Return `items` without `value` (new array, original untouched). */
export function resolveRemove(value: string, items: string[]): string[] {
  return items.filter((item) => item !== value);
}
