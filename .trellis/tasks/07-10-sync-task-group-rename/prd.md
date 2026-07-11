# Sync Task Group 支持重命名

## Goal

让用户在 Sync 页面直接 inline 重命名自己创建的 Task Group，无需删除重建。系统内置 group（Session Backup）禁止改名。

## Background（已从代码确认）

- 领域结构 `TaskGroup { id, name, tasks }`，前后端一致（`crates/nexus-core/src/services/sync.rs:50`、`src-react/src/types/index.ts:191`）。
- 用户 group 与系统 group 在**数据层已分离**：`task_groups.system_kind` 列——用户 group `system_kind IS NULL`，系统 group `system_kind IS NOT NULL`（如 `session_backup`，常量 `SESSION_BACKUP_GROUP_ID = "system:session-backup"`，`crates/nexus-core/src/services/sync.rs`）。
- `list_task_groups()` 的 SQL 带 `WHERE system_kind IS NULL`，**只返回用户 group**（`task_lifecycle.rs`）。
- 系统 group（Session Backup）走独立 `list_session_backups` query + 独立 "System-managed records" UI section（`SyncPage.tsx`），**不进入 `TaskGroupCard`**。
- 因此「禁止系统 group 改名」前端天然成立（系统 group 根本不渲染改名入口）；后端需做防御性校验：只允许改 `system_kind IS NULL` 的 group，系统 group 视为 "task group not found"。参考 `reorder_task_groups` 用 `WHERE system_kind IS NULL` 排除系统 group 的模式（`delete_task_group` 当前未做此校验，属既有不一致，不在本任务范围）。
- `create_task_group` 的名称校验：`name.trim()` 非空，无长度上限（`task_lifecycle.rs`）。重命名沿用同一惯例。
- 统一 trim+非空校验工具：`util::required_trimmed`（`crates/nexus-core/src/services/util.rs:17`）。
- 前端 API 统一用 `invokeCommand("name", { args })`（`src-react/src/lib/api/sync.ts`）。
- 前端 React Query 既有两种刷新模式：`invalidateQueries`（create/delete）、`setQueryData` 乐观替换（`replaceTask`，`src-react/src/lib/query/sync.ts`）。
- `TaskGroupCard` 头部渲染 `group.name`，右侧 action button 区已有 Add task / Delete group / Schedule / Run group（`SyncPage.tsx:324`）。

## Requirements

### R1 后端 service：`rename_task_group`
- 在 `TaskLifecycle` 新增 `rename_task_group(group_id, name) -> TaskGroup`（`crates/nexus-core/src/services/sync/task_lifecycle.rs`），并在 `SyncService` 暴露同名透传方法（`services/sync.rs`）。
- 校验：`group_id` 经 `required_trimmed`；`name` 经 `required_trimmed` 非空（与 `create_task_group` 一致）。
- 防御系统 group：查询目标 group 时带 `system_kind IS NULL` 条件；命中系统 group 或不存在均返回 `AppError::Validation("task group not found")`（与 `update_group_schedule` 一致）。
- SQL：`UPDATE task_groups SET name = ?2, updated_at = ?3 WHERE id = ?1 AND system_kind IS NULL`；返回更新后的 group（复用 `list_task_groups().find(id)` 模式）。
- 返回类型 `TaskGroup`，`serde(rename_all = "camelCase")` 沿用。

### R2 后端 command + 注册
- `src-tauri/src/commands/sync.rs` 新增 `#[tauri::command] rename_task_group(state, group_id, name) -> AppResult<TaskGroup>`，透传到 `state.sync.rename_task_group`。
- `src-tauri/src/lib.rs` 的 `invoke_handler!` 数组注册 `commands::sync::rename_task_group`。

### R3 前端 API + Query
- `src-react/src/lib/api/sync.ts` 新增 `renameTaskGroup(groupId, name)`，`invokeCommand<TaskGroup>("rename_task_group", { groupId, name })`。
- `src-react/src/lib/query/sync.ts` 新增 `useRenameTaskGroupMutation()`，`onSuccess` 用 `setQueryData<TaskGroup[]>` 乐观替换对应 group（参考 `replaceTask` 模式）。

### R4 前端 UI：inline 编辑
- 触发：group name 旁加铅笔（✏）图标按钮，点击进入 inline 编辑（不与现有单击 toggle 冲突，单击 name 区域仍展开/折叠）。
- 编辑态：name 文本替换为 `<Input>`（`@/components/ui/primitives`），自动聚焦并选中现有文本。
- 提交：Enter 或失焦 → trim 后为空则回退并提示；与原值相同则不发请求；否则调用 mutation。
- 取消：Esc → 回退原值，不发请求。
- 失败：toast 提示（sonner + `getErrorMessage`），名称回退。
- 系统 group 不渲染此入口（天然成立，见 Background）。

### R5 测试
- `crates/nexus-core/tests/sync_service.rs` 新增：重命名成功、空名报错、不存在 group 报错、系统 group 报错（用 Session Backup 的固定 id 触发防御分支）。

## Acceptance Criteria

- [ ] 用户 group 可通过 inline 编辑改名，保存后名称持久化并即时反映在 UI。
- [ ] 改名为空（或纯空白）时请求被拒，前端提示错误，名称回退。
- [ ] 名称未变化时不发请求。
- [ ] 系统 group（Session Backup）无改名入口；后端 `rename_task_group` 对其返回 "task group not found"。
- [ ] 不存在的 group id 返回 "task group not found"。
- [ ] `cargo test -p nexus-core` 新增测试通过；`src-tauri` 编译通过；前端 `pnpm typecheck` 通过。

## Out of Scope

- 给 `delete_task_group` 补 system_kind 防御校验（既有不一致，另议）。
- 名称长度上限、重名校验（create 也未做，保持一致）。
- 系统 group 名称可配置化。
