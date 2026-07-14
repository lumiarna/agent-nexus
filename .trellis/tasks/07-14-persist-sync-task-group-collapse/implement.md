# Implement · 持久化 Sync Task Group 折叠状态

## 有序实现清单

### 1. 数据库 schema / migration

- [x] 在 `crates/nexus-core/src/database/schema.rs` 将 current version 从 19 提升到 20。
- [x] 新建 schema 的 `task_groups` 增加 `collapsed INTEGER NOT NULL DEFAULT 0`。
- [x] 新增 `migrate_to_v20`，为既有 `task_groups` 增加同字段，并在迁移分发处接入。
- [x] 增加 schema migration 测试，确认旧数据库迁移后字段存在且默认值为 0。

### 2. Core Sync domain

- [x] `crates/nexus-core/src/services/sync.rs` 的 `TaskGroup` 增加 `collapsed: bool`。
- [x] `task_lifecycle.rs::list_task_groups` 查询并返回 `collapsed`。
- [x] create/list/rename/reorder/task mutation 的完整 group 返回路径保留该字段。
- [x] 新增 `TaskLifecycle::set_task_group_collapsed` 与 `SyncService` 透传；SQL 带 `system_kind IS NULL`，系统 group/不存在 id 返回 `task group not found`。
- [x] 在 `crates/nexus-core/tests/sync_service.rs` 覆盖默认展开、折叠 round-trip、展开 round-trip、不存在 group、系统 group 防御。

### 3. Tauri command

- [x] 在 `src-tauri/src/commands/sync.rs` 增加薄 command `set_task_group_collapsed`。
- [x] 在 `src-tauri/src/lib.rs` 注册 command。

### 4. Frontend contract/query

- [x] `src-react/src/types/index.ts` 的 `TaskGroup` 增加 `collapsed: boolean`。
- [x] `src-react/src/components/sync/systemRecords.ts` 为 Session Backup 构造 `collapsed: false`。
- [x] `src-react/src/lib/api/sync.ts` 增加 `setTaskGroupCollapsed`。
- [x] `src-react/src/lib/query/sync.ts` 增加 mutation，成功后替换对应 group cache。

### 5. SyncPage UI

- [x] 删除 `openGroups` React state 与 `toggleGroup` 的 map 逻辑，渲染改为 `open={g.collapsed !== true}`。
- [x] 点击 group header 时对 TaskGroup cache 做乐观 collapsed 更新，再调用 mutation；失败恢复完整快照并 toast。
- [x] 不修改 Session Backup 的 `openSec.backup`；改名、排序、任务增删/执行路径保留 collapsed 字段。

### 6. 验证

- [x] Rust 格式化：`pnpm rust:fmt`。
- [x] Rust 测试：`cargo test -p nexus-core --test sync_service`，66 passed。
- [x] Tauri：`cargo check -p agent-nexus` 通过。
- [x] 前端：`cd src-react && pnpm typecheck`。
- [x] 前端单元测试：`cd src-react && pnpm test:unit`，51 passed。
- [x] `git diff --check` 通过。

## 风险点 / 回滚点

- 迁移同时覆盖 fresh schema 与 v19 → v20，避免新安装和既有数据库行为不一致。
- 所有返回 `TaskGroup` 的 Rust 构造点、前端 `TaskGroup` 构造点均已补 `collapsed`。
- 更新缓存时保留完整 group；失败回滚操作前完整 group 快照。
- 新 command 的 SQL 带 `system_kind IS NULL`，不会改变 Session Backup 系统记录。
- 回滚不删除 v20 列；代码回退后旧版本仍可读取数据库其余数据。

## 已知的非本任务失败

- 完整 `pnpm rust:test` 中 `provider_trigger` 的 1 个 Codex 网络环境测试失败。
- `pnpm rust:lint` 存在既有 Clippy 警告。
- `pnpm test:component` 中 Provider connection forms 的 3 个既有测试失败。
