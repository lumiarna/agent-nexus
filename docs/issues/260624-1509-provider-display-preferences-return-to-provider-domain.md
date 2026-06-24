# Provider display preferences return to Provider domain

## What to build

Move Provider display preferences back into the `Provider` domain model instead of storing them as loose app settings. The target state is that `sort_index`, `card_visible`, and `tray_visible` are read and written as Provider-owned state through a dedicated Provider-facing service / command path, so the Provider page no longer has to stitch together domain data from `providers` plus UI-only preference state from `settings`.

This ticket is about correcting the data ownership boundary, not about redesigning the Provider page.

## Why

The glossary already defines `Provider Display Preferences`, `Surface Preference`, and `Card Visibility` as Provider-related concepts. The database schema also already reserves `providers.sort_index`, `providers.card_visible`, and `providers.tray_visible` for that state.

Keeping Provider ordering or visibility in `settings` creates three problems:

- It splits one Provider concept across two storage models.
- It makes future Provider preference queries harder because the page must merge two sources of truth.
- It weakens the domain boundary: a Provider preference starts looking like an arbitrary app-wide key/value rather than Provider-owned state.

## Acceptance criteria

- [ ] Provider ordering is persisted through Provider-owned storage, not through a generic `settings` key.
- [ ] `card_visible` and `tray_visible` are exposed through the same Provider-facing read/write path as ordering.
- [ ] The frontend can load Provider display state from one Provider query shape instead of merging `providers` with ad hoc preference queries.
- [ ] Reordering Provider cards persists across refresh / restart.
- [ ] Toggling Provider card visibility persists across refresh / restart.
- [ ] Toggling Provider tray visibility persists across refresh / restart.
- [ ] Existing built-in providers and OpenCode custom providers still render in a stable order, with newly discovered providers appended deterministically when no explicit preference exists yet.
- [ ] Tests cover at least one end-to-end persistence path for reorder plus one visibility preference path.

## Suggested shape

- Introduce a dedicated Provider service rather than continuing to extend `AppConfigService`.
- Treat display preferences as Provider state, not as generic application config.
- Reuse the existing `providers` table columns where they already match the glossary and schema.
- Keep quota polling concerns separate from display preference persistence; they may share a page, but they are not the same subdomain.

## Out of scope

- No redesign of Provider card layout or drag UX.
- No changes to quota polling adapter logic.
- No tray metric global setting redesign; `TrayMetric Mode` remains a global setting.
- No attempt to over-generalize this into a cross-asset preference framework.

## Notes

The minimal `settings`-based fix for Provider card order is acceptable as a short-term patch, but this ticket exists to remove that architectural drift and converge implementation back to the documented domain model.
