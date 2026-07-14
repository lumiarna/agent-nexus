# 持久化 Sync Task Group 折叠状态

## Goal

用户在 Sync 页面展开或折叠 Task Group 后，再次进入页面或重启应用时仍能看到上次选择的状态。

## Background

- `SyncPage.tsx` 当前以 `openGroups` React state 保存折叠状态，页面卸载或应用重启后丢失。
- Sync 的顺序属于任务领域数据：Group 顺序存于 `task_groups.sort_index`，Group 内 Task 顺序存于 `tasks.sort_index`，并由 `TaskLifecycle` 的 reorder 方法持久化。
- 折叠状态同样属于单个 Task Group 的领域属性，应存于 `task_groups` 记录，而不是通用 `settings` 表。
- 当前 `task_groups` 没有折叠字段，`TaskGroup` 返回模型也没有该字段；Session Backup 是独立系统 section，继续使用 `openSec.backup`，不纳入本次持久化。

## Requirements

- 在 `task_groups` 中持久化每个自定义 Task Group 的折叠状态，并通过现有 Sync 数据链路返回前端。
- 新建 group 默认展开；已有数据库通过迁移获得同样的默认值。
- 点击折叠/展开后立即更新 UI，并持久化到对应 group；持久化失败时恢复之前状态并提示错误。
- group 重命名、排序、任务增删或任务执行不得重置折叠状态；删除 group 时其字段随记录删除。
- Session Backup 系统 section 继续默认折叠，不使用自定义 Task Group 的折叠字段。

## Acceptance Criteria

- [ ] 用户折叠或展开任意自定义 Task Group 后，离开并重新进入 Sync 页面，状态保持一致。
- [ ] 应用重新启动后，自定义 Task Group 的折叠状态仍保持一致。
- [ ] 新建 group 默认展开，删除 group 不影响其他 group。
- [ ] group 重命名、排序、任务编辑/执行不会使其折叠状态恢复为默认值。
- [ ] 持久化失败时 UI 回滚到操作前状态并显示可诊断错误。
- [ ] Session Backup 系统 section 仍默认折叠，且不受自定义 group 折叠状态影响。
- [ ] 数据库迁移、后端 Sync 测试、前端类型检查和相关前端测试通过。

## Out of Scope

- 不改变 Task Group 的执行、排序或其他业务语义。
- 不持久化 Session Backup 系统 section 的折叠状态。
- 不使用 `settings` 表或新增通用全局状态库。
