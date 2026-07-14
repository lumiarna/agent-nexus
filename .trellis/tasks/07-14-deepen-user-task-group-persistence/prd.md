# 深化用户 Task Group 持久化规则

## Goal

在 `TaskLifecycle` 内深化用户 Task Group 持久化 module，集中用户/系统托管记录的写权限、聚合返回与排序不变量，同时保持 Sync 的外部 interface 稳定。

**推荐强度：Strong**

## Evidence

- `crates/nexus-core/src/services/sync/task_lifecycle.rs:125-580` 同时包含 Task Group list、折叠、删除、重命名、添加 Task、Schedule 与排序。
- 折叠、重命名、Group 排序显式限制 `system_kind IS NULL`；删除 Group、添加 Task、Group Schedule、Task 排序等路径只检查 ID 是否存在。
- `TaskLifecycle` 已是 deep module；问题是内部新增 Group 规则继续散落，而非需要新的外部 orchestration seam。

## Requirements

1. 以 `CONTEXT.md` 为准集中动作权限：系统托管 Session Backup 允许 Run、Run Group 与逐 Task Schedule；不允许新增、删除或拖拽排序。
2. 用户/系统托管 Task Group 的加载与写权限规则只定义一次，不能依赖 UI 隐藏按钮维持不变量。
3. 保持 `TaskLifecycle` 的外部 leverage，不把内部深化变成新的 pass-through 层。
4. 事务、placement 回滚、完整聚合返回和排序校验继续由 core module 维护。
5. 不重新设计已有 Transfer seam，也不新增数据库 adapter。
6. 实现前比较至少两种内部 module 组织方式，本任务当前不指定最终 interface。

## Acceptance Criteria

- [ ] 删除系统 Group、向系统 Group 新增 Task、删除系统 Task、拖拽重排系统 Group/Task 均由 core interface 拒绝。
- [ ] Session Backup 的 Run、Run Group 与逐 Task Schedule 仍按领域约束可用。
- [ ] 用户 Task Group 的折叠、重命名、删除、添加、Schedule 与排序行为保持兼容。
- [ ] 表驱动契约测试覆盖每类动作对用户 Group 与系统 Group 的允许/拒绝结果。
- [ ] 写权限和存在性判断不再在每个方法中以不同 SQL 形式重复。
- [ ] deletion test 复核表明删除内部深化 module 会让权限和排序不变量重新散布到多个写路径。

## Out of Scope

- 抽取跨领域 orchestration 目录。
- 修改 Session Backup 的领域能力。
- 修改 Sync Task 的 Direction、Action 或 Transfer 语义。
