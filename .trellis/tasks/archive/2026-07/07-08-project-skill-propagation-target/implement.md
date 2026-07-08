# Implementation Plan

## Pre-development Context

Before editing code, load `trellis-before-dev` and read the specific guideline docs it routes to for:

- `crates/nexus-core` backend database/service changes.
- `src-tauri` command exposure.
- `src-react` frontend API/query/component changes.
- Shared guide: cross-layer thinking, because this changes Rust DTOs, Tauri commands, TS types, query hooks, and UI.

## Ordered Checklist

### 1. Backend schema

- Add schema v19 migration in `crates/nexus-core/src/database/schema.rs`.
- Create `skill_project_distributions` table with `(skill_id, target_project_id, agent)` primary key.
- Add tests for migration/table availability if existing schema tests cover versions.

### 2. Core service model

- Extend `Skill` DTO with optional projection fields, likely:
  - `canonicalSkillId?: String`
  - `placementScope?: String`
  - `placementProjectId?: String`
  - `sourceProjectId?: String`
- Ensure existing canonical rows remain backward-compatible.
- Add helpers for target Project lookup and target Project skill path resolution.

### 3. Cross-project distribution service functions

- Add service methods:
  - `set_project_skill_project(skill_id, target_project_id, default_agent, enabled)`
  - `set_project_skill_target(skill_id, target_project_id, agent, enabled)`
- Validate:
  - source Skill exists and `source_kind = project_custom`.
  - source Skill has a source `project_id`.
  - target Project exists and is active.
  - target Project differs from source Project.
  - Agent is skill-capable.
- Use existing `distribution::write_target` pattern with `create_managed_directory_link` / `remove_managed_directory_link_if_present`.
- For project cancellation, remove all target Project Agent placements for that skill.

### 4. Scan/list projection

- Keep canonical scanning unchanged except for validating `skill_project_distributions` placements after scan.
- Extend `list_skills` to append incoming Project projection rows for target Projects that have at least one live `target` placement.
- Ensure projection row cells come from `skill_project_distributions` and contain only `target / none`.
- Ensure projection rows use target Project as `projectId` and source Project as `sourceProjectId` for tooltip lookup.

### 5. Tauri commands

- Add commands in `src-tauri/src/commands/skills.rs`.
- Register commands in `src-tauri/src/lib.rs`.
- Return updated skill list or updated projection row consistently; simplest front-end can invalidate/refetch skills after these mutations.

### 6. Frontend types/API/query

- Extend `src-react/src/types/index.ts` `Skill` with optional projection fields.
- Add API methods in `src-react/src/lib/api/skills.ts`.
- Add React Query mutations in `src-react/src/lib/query/skills.ts`; invalidate/refetch `skillKeys.all` and `projectKeys.all` after cross-project changes.
- Keep existing `setSkillTarget` behavior for Global propagation and normal Agent Matrix rows.

### 7. Frontend UI

- Replace Project custom source `PropagateToGlobal` toggle with a propagation menu/control.
- Menu targets:
  - Global.
  - Other active Projects.
- Source Project row operations:
  - Global enabled/disabled mirrors current all-Global-placement behavior.
  - Project enabled/disabled calls new cross-project project-level command.
- Target Project incoming row:
  - Render like current Global Project source row.
  - Agent Matrix toggles call cross-project single-agent command.
  - Source badge/tooltip reuse current Project source style.
- Ensure ordinary Agent-sourced Project Skill rows do not get cross-project propagation entry.

### 8. Tests

Backend tests in `crates/nexus-core/tests/skill_service.rs` or a focused new file:

- Existing Global propagation test still passes.
- Project custom Skill propagates to another Project using default Agent path.
- Target Project incoming projection row appears with default Agent `target`, no `source` cells.
- Target Project row can fan-out to another Agent project skills dir.
- Removing last Agent placement removes incoming projection row.
- Source-side cancellation removes all target Project placements.
- Rescan does not create new canonical Skill from target Project placement.
- Existing non-managed target path conflict fails and does not overwrite.

Frontend validation:

- `cd src-react && pnpm typecheck`.
- Add/adjust component tests only if existing test harness covers SkillRow/menu behavior.

Rust validation:

- Prefer targeted tests first.
- On Windows, do not run bare `cargo test -p nexus-core`; use `pnpm rust:test` or the SQLite wrapper per `GOTCHAS.md`.
- For single-file formatting, prefer `cargo fmt --all -- <path>`; final validation can use `pnpm rust:fmt`.

## Rollback Points

- Schema migration v19 is additive; rollback during development can drop the new table before release.
- Frontend menu can fall back to existing `PropagateToGlobal` if backend command wiring is incomplete, but final acceptance requires both Global and Project targets.
- If projection DTO becomes too risky, alternative is a separate `SkillPlacementRow` DTO; however that is a larger UI/API split and should be avoided unless necessary.
