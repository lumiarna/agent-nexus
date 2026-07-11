# Design · Sync Task Group 重命名

## 架构与边界

复用现有 Sync 四层链路，仅新增一条「rename」纵切面，不改变既有 group / session-backup 数据分离：

```
SyncPage (TaskGroupCard)  ──inline edit──▶  useRenameTaskGroupMutation
        │                                            │
        │                                            ▼
        │                                  syncApi.renameTaskGroup
        │                                            │ invokeCommand
        ▼                                            ▼
  React Query setQueryData (乐观替换 group)   rename_task_group (command)
                                                     │ 透传
                                                     ▼
                                          SyncService::rename_task_group
                                                     │ 透传
                                                     ▼
                                          TaskLifecycle::rename_task_group
                                                     │ SQL UPDATE
                                                     ▼
                                          task_groups (system_kind IS NULL)
```

边界：重命名只动 `task_groups.name` + `updated_at`，不触碰 `tasks`、placements、file_state、settings。

## 数据流与契约

### 后端契约
- 入参：`group_id: String`, `name: String`（command 参数名 camelCase 经 Tauri 桥接）。
- 出参：`TaskGroup`（更新后的完整 group，含 tasks）。
- 错误（`AppError::Validation`，message 文案）：
  - `task group id is required`（id 空白，来自 `required_trimmed`）
  - `task group name is required`（name 空白）
  - `task group not found`（id 不存在 **或** 命中系统 group）

### SQL（防御系统 group）
```sql
UPDATE task_groups
SET name = ?2, updated_at = ?3
WHERE id = ?1 AND system_kind IS NULL
```
- `rows_affected == 0` → 走 "task group not found" 分支（覆盖「不存在」与「系统 group」两种情况，单语句即可，无需先 SELECT）。
- 成功后用 `list_task_groups().into_iter().find(|g| g.id == group_id)` 返回最新 group（与 `create_task_group` / `add_task` 返回路径一致，保证 tasks 一并返回）。

### 前端契约
- `syncApi.renameTaskGroup(groupId, name): Promise<TaskGroup>` → `invokeCommand<TaskGroup>("rename_task_group", { groupId, name })`。
- `useRenameTaskGroupMutation()`：
  ```ts
  onSuccess: (updated) => {
    queryClient.setQueryData<TaskGroup[]>(syncKeys.taskGroups, (groups) =>
      groups?.map((g) => (g.id === updated.id ? updated : g)),
    );
  }
  ```
  走乐观替换（与 `replaceTask` / `useReorderTasksMutation` 同构），不触发整表 invalidate，避免折叠态丢失。

## 前端 inline 编辑状态机（TaskGroupCard 头部）

```
状态：editing: boolean  |  draft: string

idle (editing=false)
  ├─ 点击 ✏  → editing=true, draft=group.name, autofocus+select
editing (editing=true)
  ├─ Enter   → commit()
  ├─ blur    → commit()
  ├─ Esc     → editing=false (回退，不发请求)
commit():
  trimmed = draft.trim()
  if trimmed === group.name → editing=false (无变化)
  else if trimmed === ""    → toast("名称不能为空"), editing=false (回退)
  else                      → mutate({groupId, name:trimmed});
        onSuccess: editing=false
        onError:    toast(getErrorMessage(e)); editing=false (回退原值)
```

- 铅笔按钮 `onClick` 必须 `e.stopPropagation()`（否则冒泡触发头部 `onToggle` 折叠）。
- 编辑态 `<Input>` 的 `onClick` 同样 `stopPropagation`，且 keydown Enter/Esc 也 `stopPropagation` 避免 toggle。
- 编辑态隐藏铅笔按钮；非编辑态显示铅笔 + name 文本。

## 兼容性 / 迁移

- 纯新增列已存在数据的 UPDATE，无 schema 变更，无迁移。
- 新增 Tauri command 需前端重新生成 binding 吗？现有 `invokeCommand` 为字符串式调用，不依赖 codegen，无需额外步骤。
- 系统 group（Session Backup）走独立 query/section，不受影响。

## 取舍

- **单 UPDATE 语句 + rows_affected 判定** vs「先 SELECT 再 UPDATE」：前者更省一次查询，且 `WHERE system_kind IS NULL` 一并完成防御，逻辑更简。采用前者。
- **乐观 setQueryData** vs **invalidateQueries**：乐观替换保留用户折叠/展开态与排序，体验更连贯（与现有 reorder/renameTask 同构）。采用乐观。
- **失焦提交** vs **仅 Enter 提交**：失焦提交更符合 inline 编辑直觉，配合 Esc 取消兼顾「想取消就 Esc」。采用失焦提交。
- 不引入名称长度上限 / 重名校验：与 `create_task_group` 对齐，避免出现「创建允许、改名不允许」的不一致。
