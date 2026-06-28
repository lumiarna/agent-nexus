# ProjectDetailView 订阅 scan query 的副作用 — 与未来"打开只读"语义冲突

## 问题

为了让 Project 列表的 `p.sessions` / `p.skills` / `p.prompts` 数字跟着 scan 同步，我们在 `useLocalSessionsQuery` / `useCloudSessionsQuery` / `useSkillsQuery` / `usePromptsQuery` 四个 query hook 的 queryFn 内加了 `invalidateQueries({ queryKey: projectKeys.all })`。

而 `ProjectDetailView` 在 mount 时已经订阅了这四个 query（`src-react/src/components/project/ProjectDetailView.tsx:71-75`）：

```ts
const skillsQuery = useSkillsQuery();
const promptsQuery = usePromptsQuery();
const localSessionsQuery = useLocalSessionsQuery();
const cloudSessionsQuery = useCloudSessionsQuery();
```

合在一起产生一个未被声明的副作用链：**打开任意 Project 详情页 = 触发一次 scan_skills + scan_prompts + scan_local_sessions + scan_cloud_sessions + list_projects**。

## 现状

事实：

- 打开 Project 详情：~5 个 IPC 调用（4 个 scan + 1 个 list），全部走 Rust backend。
- 用户在 Session / Skill / Prompt 页面点 Refresh：触发对应 scan + list_projects（功能正确）。
- 用户在 Project 详情页：mount 即触发 4 个 scan + list_projects（副作用链，不是显式意图）。
- 后端 `scan_*` 是全量扫描所有已收录 Project，不是单 Project 局部刷新——所以"打开 Project A 的详情"会扫 Project B/C/... 的 session 目录，浪费明显。

后端对照：

- `services/sessions.rs:202 scan_local_sessions` — 全量 scan。
- `services/skills.rs` / `services/prompts.rs` 同理（`scan_skills` / `scan_prompts` 命令）。
- `services/projects.rs:111 list_projects` — 全量 list，扫描每个 Project 的 sessions_dir / custom_skills_dirs。

## 决策待定

未来若要让"打开 Project 详情"是真正的"只读、零副作用"动作，需要拆开两个 query：

### 方案 A：拆 `scan` 和 `list` 为两个 query hook

- 当前实现：scan-only（`scan_skills` / `scan_prompts` / `scan_local_sessions` / `scan_cloud_sessions`），list 隐藏在 scan 里。
- 拆开后：
  - `useScanXxxMutation` 显式做副作用（保持 issue `260628-1851` 中的方案 A 重构）。
  - `useXxxQuery` 仅走后端 `list_*` 命令。
- ProjectDetailView 只订阅 query（只读），不订阅 mutation。
- SessionPage / SkillPage / PromptPage 各自暴露显式的 `useScanMutation` 给 Refresh 按钮调用。

后端与前端 `list_*` 命令都已存在，无需新增：

- 后端：`services/sessions.rs:108 list_local_sessions` / `:112 list_cloud_sessions`、`services/skills.rs:118 list_skills`、`services/prompts.rs:134 list_prompts`。
- 前端：`api/sessions.ts:5 listLocal` / `:9 listCloud`、`api/skills.ts:11 list`、`api/prompts.ts:11 list` 已 export 但当前没有 query hook 调用它们。

优点：

- ProjectDetailView mount = 0 副作用（5 个 IPC 降到 5 个 list）。
- "打开详情是只读动作"的语义得到代码结构保证。

缺点：

- 工作量与 issue `260628-1851` 的方案 A 相当，需要 4 个 mutation + 调用方改造 + 新测试。
- 后端 `list_*` 读的是已持久化的 `session_index` / skills / prompts 表，**不重新扫盘**——所以"scan 后 list" 与 "scan 直接返回" 在同一 session 内拿到的数据等价；但跨进程/跨重启时 `list_*` 依赖上一次 scan 的索引新鲜度，必须设计好"什么时候必须触发一次 scan"（如启动时、Project 录入时、Custom Source 变更时）。这是一个独立的产品决策。

### 方案 B：保留当前结构，给 ProjectDetailView 单独的只读 query

- 复制 4 个 hook 为 `useXxxListQuery`（queryFn 走 `list_*`）和 `useXxxScanQuery`（queryFn 走 `scan_*`，带 invalidate）。
- ProjectDetailView 订阅 List 版；其它页面订阅 Scan 版。

优点：

- 不改 call site（ProjectDetailView 仍然订阅同样的"4 个 hook"，只是订阅不同的 queryKey）。
- 干净一刀切。
- 后端 `list_*` 都已存在（见方案 A），无新依赖。

缺点：

- queryKey 翻倍，cache 占用翻倍。

### 方案 C：维持现状，记账即可

- ProjectDetailView 的 mount 现在是重操作，但用户感知不到明显延迟。
- 后端 scan 是内存/磁盘扫一遍，量级可控（个人桌面场景下 Project 数量 < 1000）。

优点：

- 零工作量。
- 配合 issue `260628-1851` 中的范式债记账，统一处理。

缺点：

- mount 一份 Project 详情 = 一次完整的"refresh all"等价操作，语义上不对等。
- 未来如果后端 scan 变重（比如 Project 数量级跳到 10000+）会变成可感知的延迟。

## 建议

**维持现状（方案 C），但与 issue `260628-1851` 合并为一个重构任务**。

理由：

- 当前是 MVP 阶段，个人桌面 Project 数量有限，mount 触发全量 scan 在量级上不痛。
- 拆 scan/list 的语义改造与 `260628-1851` 的范式改造完全重叠（都要新增 mutation），合并为一个 PR 更经济。
- 写 issue 把副作用链记下来，比悄悄放进代码好——下个看代码的人能在 `docs/issues/` 找到"这不是 bug 而是已知副作用"。

## 未来实现约定

如果未来决定走方案 A：

- 后端核对：确认 `services/skills.rs` / `services/prompts.rs` 已有 `list_skills` / `list_prompts` 命令（已存在 `list_local_sessions` / `list_cloud_sessions`）；如有缺失要先补。
- 前端：与 `260628-1851` 的方案 A 重构共享 mutation 化路径。
- ProjectDetailView 迁移：4 个 `use*Query` 调用保留 queryKey，但 queryFn 走 `list_*`（mutation 触发的 scan 在别处发生）。
- 测试：保留 `tests/component/scanInvalidatesProjects.test.tsx`，新增 `tests/component/projectDetailViewMount.test.tsx` 验证 mount 不触发额外副作用。

工作量估计：与 `260628-1851` 重叠，单独做约 2-3 小时（含后端核对）。

## 验收标准

- [ ] 本 issue 不阻塞当前功能上线。
- [ ] 未来重构时，要在 plan 里明确"ProjectDetailView mount 是 0 副作用"作为验收点。
- [ ] 重构后保留 `scanInvalidatesProjects.test.tsx`，并新增"ProjectDetailView mount 不触发额外副作用"的测试。
- [ ] 后端 `list_*` 命令均已存在（`services/sessions.rs:108/112`、`services/skills.rs:118`、`services/prompts.rs:134`）；前端 `list` / `listLocal` / `listCloud` API 也已 export，重构时直接接入 query 即可。

## 备注

- 当前副作用链是个**意外的功能正确性收益**：ProjectDetailView mount 让 Project List 的数字保持新鲜（因为 mount 触发 scan + invalidate projects）。这是个白嫖的副作用，不应作为设计意图，但短期享受即可。
- 后端 `scan_*` 全量扫描的设计本身有讨论空间。如果未来 scan 变重，issue `260628-1851` 的方案 A 重构 + 后端引入"按 project_id 局部 scan"是两个独立但相关的优化。
- 这两个 issue（`260628-1851` 与本 issue）合起来是"scan → projects invalidate"这次改动的完整技术债务清单，建议未来一起处理。