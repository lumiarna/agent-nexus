# Design · 持久化 Sync Task Group 折叠状态

## 架构与边界

折叠状态作为 Task Group 领域字段，复用 Sync 的 `task_groups` 数据链路：

```text
SyncPage
  └─ useSetTaskGroupCollapsedMutation
       └─ syncApi.setTaskGroupCollapsed
            └─ set_task_group_collapsed (Tauri command)
                 └─ SyncService / TaskLifecycle
                      └─ task_groups.collapsed
```

- `TaskGroup` 增加 `collapsed: bool`，由 `list_task_groups`、create/rename/reorder 等返回完整 group 时携带。
- UI 不再维护独立的 `openGroups` map；React Query 中的 TaskGroup 是唯一数据来源，渲染使用 `open={g.collapsed !== true}`。
- Session Backup 仍通过 `sessionBackupsToTaskGroup` 构造仅用于展示的 group，固定 `collapsed: false`；其 `openSec.backup` 状态继续是临时状态且默认折叠。

## 数据库与迁移

- 在新建 schema 的 `task_groups` 表增加：
  `collapsed INTEGER NOT NULL DEFAULT 0`。
- 当前 schema version 为 19，新增 `migrate_to_v20`，执行：
  `ALTER TABLE task_groups ADD COLUMN collapsed INTEGER NOT NULL DEFAULT 0`。
- SQLite 的 0/1 通过 rusqlite 读取为 `bool`；旧数据自动得到 0（展开）。不新增 settings key。

## 数据契约

### Core

`TaskGroup`：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
    pub tasks: Vec<Task>,
}
```

`TaskLifecycle::list_task_groups` 的 group 查询读取 `id, name, collapsed`，并把该字段填入返回对象。

新增 `set_task_group_collapsed(group_id, collapsed) -> AppResult<TaskGroup>`：

- trim/校验 group id。
- `UPDATE task_groups SET collapsed = ?2, updated_at = ?3 WHERE id = ?1 AND system_kind IS NULL`。
- 更新 0 行返回 `task group not found`，防止修改 Session Backup 系统 group。
- 成功后复用 `list_task_groups` 返回完整 group。

### Tauri / frontend

- 新增 command `set_task_group_collapsed(state, group_id, collapsed) -> AppResult<TaskGroup>` 并注册到 `invoke_handler`。
- `syncApi.setTaskGroupCollapsed(groupId, collapsed)` 调用该 command。
- `useSetTaskGroupCollapsedMutation` 成功后用 `setQueryData` 替换对应 group，保留 tasks 与其他字段。

## UI 数据流与失败处理

1. `useTaskGroupsQuery` 返回包含 `collapsed` 的 group；新建/旧 group 未折叠时为 `false`。
2. `toggleGroup` 读取当前 group，计算 `nextCollapsed`，先将 React Query cache 乐观更新为该值。
3. mutation 失败时将该 group 恢复到操作前的完整快照，并 toast 错误；不会阻断后续 UI 操作。
4. mutation 成功时以后端返回的完整 group 覆盖 cache，确保数据库结果为真相源。
5. 重命名、排序、任务增删/执行沿用已有返回或 cache 更新路径，并保留 `collapsed` 字段；删除由数据库级联移除 group。

## 关键取舍

- **`task_groups` 字段 vs `settings`**：折叠状态与 group 生命周期、group id 和 Sync CRUD 同域，放在实体记录中不会产生另一个按 id 维护的偏好索引，也天然随删除清理。
- **`bool` 字段 vs 单独偏好表**：单个 group 只有一个低基数状态，SQLite `INTEGER NOT NULL DEFAULT 0` 最小且易迁移。
- **缓存乐观更新 vs 等待 command**：沿用现有 Sync reorder 的即时交互；失败回滚完整 group 快照。
- **新增 command vs 扩展 reorder payload**：折叠与排序是独立操作，单独 command 不会把 UI 展示动作混入排序语义。

## 兼容性 / 回滚

- v19 数据通过 v20 迁移自动展开；新安装的 schema 直接包含字段。
- 前端与后端需同步发布；旧版前端忽略新返回字段，旧版后端不支持新 command，因此本功能需随同版本发布。
- 回滚代码不删除 `collapsed` 列；旧版本可继续读取其余字段，数据库数据不受破坏。
