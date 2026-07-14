# 深化 Sync Task Group cache mutation 模块

## Goal

深化前端 Sync query mutation module，使 Task Group 用户动作的 cache transition、乐观更新、权威响应替换和失败回滚隐藏在较小 interface 后，提升页面 locality 与测试 leverage。

**推荐强度：Strong**

## Evidence

- `src-react/src/lib/query/sync.ts:51-173` 已处理部分成功后的 cache 更新。
- `src-react/src/components/sync/SyncPage.tsx:748-959` 又直接掌握 `syncKeys`、snapshot、乐观更新和 rollback。
- 删除、运行、排序、折叠等路径存在 query module 与页面 module 重复写 cache 的情况。
- `cbc77ad`、`98ca3db`、`7d53037` 三次连续 Task Group 变化都穿透相同层次，证明不是一次性重复。

## Requirements

1. 现有 React Query seam 保持为 server state 真相源。
2. SyncPage 不再为 Task Group mutation 自行管理 query key、cache snapshot、成功替换与失败回滚。
3. mutation module 的 interface 表达用户意图，而不是暴露缓存实现顺序。
4. 保持拖拽即时反馈、折叠即时反馈、运行状态更新与现有 toast 行为。
5. 不增加平行状态容器，也不引入新的状态库或假想 adapter。
6. 实现前用 design-it-twice 比较“深化现有 hooks”与“领域 mutation module”等形状，本任务不预定最终 interface。

## Acceptance Criteria

- [ ] SyncPage 不直接使用 `syncKeys.taskGroups` 写入上述 mutation 的缓存。
- [ ] 排序、折叠、删除、运行等操作不存在 query hook 与页面重复应用同一权威响应。
- [ ] 纯测试覆盖成功、失败回滚、连续 mutation 与过期响应不能覆盖较新状态。
- [ ] Task Group 与 Session Backup 需要同步更新时，其 cache 关系集中在同一 locality。
- [ ] 现有 UI 交互、排序结果、折叠持久化和运行状态保持兼容。
- [ ] deletion test 复核表明删除深化后的 module 会让缓存协议重新扩散到页面调用者。

## Out of Scope

- 修改 Rust Sync Task Group 规则。
- 重做 Sync 页面视觉结构。
- 引入 Zustand 等额外状态库。
