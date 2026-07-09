# Implementation Plan

## Pre-development Context

开始写代码前加载 `trellis-before-dev`，并按其路由读取相关规范：

- `crates/nexus-core` backend service / tests。
- `src-react` frontend helper / component / query。
- Shared cross-layer guide，因为该任务涉及 Skill / Project / Distribution 的跨层契约。

## Ordered Checklist

### 1. Frontend target list

- 修改 `src-react/src/components/skill/propagation.ts`：
  - 更新注释：目标列表为 `Global + every active Project including the source Project`。
  - 移除 `if (project.id === sourceProjectId) continue;`。
  - 保持 enabled 状态由 matching projection row 的 `canonicalSkillId + placementScope + placementProjectId` 推导。
- 如已有 propagation helper 单元测试，增加/调整用例覆盖 source Project 出现在 targets 中。

### 2. Backend validation

- 修改 `crates/nexus-core/src/services/skills.rs`：
  - `set_project_skill_project` 不再拒绝 `context.source_project_id == target_project_id`。
  - `set_project_skill_target` 不再拒绝 `context.source_project_id == target_project_id`。
  - 保留 `project_skill_context`、`project_root`、`require_agent`、managed link conflict 等既有校验。
- 更新相关注释，避免继续声明 `target_project_id must differ from source Project`。

### 3. Backend projection / scan verification

- 检查 `list_skills` projection row grouping 是否允许 `target_project_id == source_project_id`。
- 确保 canonical source row 和 current-project projection row 使用不同 `id`，且 `canonicalSkillId` 指向真实 source skill id。
- 确保 scan/discover 仍跳过 symlink/junction placement。

### 4. Backend tests

在 `crates/nexus-core/tests/skill_service.rs` 或相邻现有测试文件中增加/调整测试：

- Project custom Skill 可以 propagate 到 source/current Project。
- 目标 path 是 source Project 下默认 Agent 的固定 project skills dir，不是 `custom_skills_dirs`。
- list 后同一 Project 能看到 canonical source row 与 current-project projection row，projection row 的 cells 有默认 Agent `target`。
- current Project projection row 可 fan-out 到另一 Agent，并可单独移除。
- source-side cancel 当前 Project target 会删除该 skill 在当前 Project 下的所有 managed placements。
- target path 已有真实目录或非托管目录时失败，不覆盖。
- rescan 后 current Project placement 不会成为新的 canonical Skill；canonical source 不重复。
- 现有 Global 与其他 Project propagation 测试继续通过。

### 5. Frontend UI smoke check

- 确认 Project custom Skill canonical row 的 `Propagate to…` modal 出现当前 Project。
- 确认普通 Agent-sourced Project Skill 不出现该入口。
- 确认 current Project target enabled 状态与 target Agents 能从 projection row 显示。

### 6. Validation commands

- 前端：`cd src-react && pnpm typecheck`。
- Rust 格式：优先 `cargo fmt --all -- <path>`；最终可跑 `pnpm rust:fmt`。
- Rust 测试：Windows 下不要跑裸 `cargo test -p nexus-core`；使用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core <test-name>`。
- 若 `with-sqlite` 因运行中的旧 `agent-nexus` 占用 `sqlite3.dll` 失败，按 `GOTCHAS.md` 使用手动 `SQLITE3_LIB_DIR` + PATH 绕过，不杀用户进程。

## Rollback Points

- 前端恢复 source Project exclusion 即可隐藏入口。
- 后端恢复 source/target 不同校验即可阻止写入。
- 本任务不做 schema migration，回滚不需要数据库降级。

## Review Gate Before `task.py start`

- `prd.md`、`design.md`、`implement.md` 已覆盖当前 Project 作为 target 的要求与风险。
- `implement.jsonl` / `check.jsonl` 至少包含真实规范/研究条目。
- 用户确认进入实现后，再运行 `task.py start`。
