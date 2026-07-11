# Implement · Sync Task Group 重命名

## 有序实现清单

> 全程参考 `design.md` 的状态机与 SQL。系统 group 防御靠 `WHERE system_kind IS NULL` + `rows_affected` 单语句完成。

### 1. 后端 service（nexus-core）
- [ ] `crates/nexus-core/src/services/sync/task_lifecycle.rs`：在 `TaskLifecycle` impl 中新增
  ```rust
  pub(super) fn rename_task_group(&self, group_id: String, name: String) -> AppResult<TaskGroup>
  ```
  - `let group_id = required_trimmed(&group_id, "task group id")?.to_string();`
  - `let name = required_trimmed(&name, "task group name")?;`
  - `let now = now_epoch_seconds()?;`
  - `conn.execute("UPDATE task_groups SET name = ?2, updated_at = ?3 WHERE id = ?1 AND system_kind IS NULL", params![group_id, name, now])?;`
  - `rows_affected == 0` → `Err(AppError::Validation("task group not found".to_string()))`
  - 成功 → `self.list_task_groups()?.into_iter().find(|g| g.id == group_id).ok_or_else(|| AppError::Internal("renamed task group was not found".to_string()))`
- [ ] `crates/nexus-core/src/services/sync.rs`：`SyncService` impl 新增透传
  ```rust
  pub fn rename_task_group(&self, group_id: String, name: String) -> AppResult<TaskGroup> {
      self.task_lifecycle.rename_task_group(group_id, name)
  }
  ```
  并在 `use ... sync::{...}` 处确认导出无需新增（`TaskGroup` 已导出）。

### 2. 后端 command + 注册（src-tauri）
- [ ] `src-tauri/src/commands/sync.rs`：
  ```rust
  #[tauri::command]
  pub fn rename_task_group(
      state: State<'_, AppState>,
      group_id: String,
      name: String,
  ) -> AppResult<TaskGroup> {
      state.sync.rename_task_group(group_id, name)
  }
  ```
- [ ] `src-tauri/src/lib.rs`：`invoke_handler!` 数组内加 `commands::sync::rename_task_group,`（放在其他 `commands::sync::*` 附近即可，顺序无要求）。

### 3. 后端单测（nexus-core）
- [ ] `crates/nexus-core/tests/sync_service.rs`：新增用例（参考既有 `create_task_group` 测试的建组方式）
  - 重命名成功：建组 → rename → `list_task_groups()[0].name == 新名`
  - 空名报错：`rename(id, "  ")` → `Err` 含 "task group name is required"
  - 不存在 id：`rename("nonexistent-uuid", "x")` → `Err` 含 "task group not found"
  - 系统 group 报错：`rename(SESSION_BACKUP_GROUP_ID, "x")` → `Err` 含 "task group not found"（`SESSION_BACKUP_GROUP_ID` 已存在于 service 常量；测试模块按需 import 或硬编码 `"system:session-backup"`）

### 4. 后端验证
- [ ] `cargo test -p nexus-core`（新增用例通过）
- [ ] `cargo check -p src-tauri`（command 注册与透传编译通过）

### 5. 前端 API（src-react）
- [ ] `src-react/src/lib/api/sync.ts`：`syncApi` 新增
  ```ts
  renameTaskGroup(groupId: string, name: string): Promise<TaskGroup> {
    return invokeCommand<TaskGroup>("rename_task_group", { groupId, name });
  },
  ```

### 6. 前端 Query（src-react）
- [ ] `src-react/src/lib/query/sync.ts`：新增
  ```ts
  export function useRenameTaskGroupMutation() {
    const queryClient = useQueryClient();
    return useMutation({
      mutationFn: ({ groupId, name }: { groupId: string; name: string }) =>
        syncApi.renameTaskGroup(groupId, name),
      onSuccess: (updated) => {
        queryClient.setQueryData<TaskGroup[]>(syncKeys.taskGroups, (groups) =>
          groups?.map((g) => (g.id === updated.id ? updated : g)),
        );
      },
    });
  }
  ```

### 7. 前端 UI（src-react · `SyncPage.tsx`）
- [ ] `TaskGroupCard` 内引入 `useRenameTaskGroupMutation` + 局部 state `editing` / `draft`（或在 `SyncPage` 主组件持有 mutation，经 props 传入回调 `onRenameGroup`，与 `onDeleteGroup` 等回调同构）。
- [ ] 头部非编辑态：在 name 文本后加铅笔图标按钮（`onClick` 需 `e.stopPropagation()`，`title="Rename group"`），样式参考既有圆角 action button。
- [ ] 编辑态：渲染 `<Input value={draft} ...>`，`autoFocus` + `onFocus={e => e.target.select()}`；`onKeyDown` 处理 Enter→commit / Esc→cancel（均 `e.stopPropagation()`）；`onBlur`→commit。
- [ ] commit 逻辑：trim；空→toast + 回退；等于原值→静默退出；否则 `mutate`，`onSuccess` 退出编辑、`onError` toast+回退。
- [ ] 主组件接线：`useRenameTaskGroupMutation()`，把 `onRenameGroup` 经 `TaskGroupCard` props 传入。

### 8. 前端验证
- [ ] `cd src-react && pnpm typecheck`（`tsc --noEmit`）
- [ ] （可选）`pnpm build` 做一次完整构建

## Validation Commands

```bash
# 后端
cargo test -p nexus-core
cargo check -p src-tauri
# 前端
cd src-react && pnpm typecheck
```

## 风险点 / 回滚点

- **系统 group 防御依赖 `system_kind IS NULL`**：务必在 UPDATE 的 WHERE 带该条件；漏掉会导致系统 group 被改名。单测用例「系统 group 报错」是关键守门。
- **乐观更新 vs 折叠态**：用 `setQueryData` 而非 `invalidateQueries`，否则会重置 `openGroups` 折叠状态（用户感知为「卡片自己折叠了」）。
- **事件冒泡**：铅笔按钮 / 编辑态 Input 的 click 与 keydown 必须 `stopPropagation`，否则触发头部 `onToggle` 折叠。
- **回滚**：本任务全为新增（service 方法 / command / api / mutation / UI），无既有代码删除；若需回退直接 `git checkout` 相关文件即可，无数据迁移。

## Follow-up（实现后自查）

- 改名后 `tasks` 列表是否完整带回（依赖 `list_task_groups().find()`，与 create/add 一致）。
- 编辑态下 `TaskGroupRow` 的拖拽/排序是否被禁用（编辑中理论上不应拖动整卡，但既有 Sortable 绑在卡外层，可接受，不阻塞）。
