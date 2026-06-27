# ProjectPage God-component 按 feature 抽 hooks 与视图组件

> 架构深化 issue（improve-codebase-architecture，前端）。推荐强度：**Worth exploring**。
> 词汇遵循 `CONTEXT.md`（Project / Stale Project / Project discovery）与 codebase-design（module / interface / depth / locality / leverage）。

## 问题

`src-react/src/components/project/ProjectPage.tsx` 单个 `ProjectPage` 组件约 **1500 行**（214 行起到文件末），持有 **21 个 useState**、15 个 useQuery。这些 state 分属 ≥6 个**彼此不共享**的 feature cluster，却全挤在一个组件作用域里：

| cluster | 相关 state（行号） | 关注点 |
| --- | --- | --- |
| 列表 + 拖拽 + 右键菜单 | `order` `hiddenIds` `menu` `251-252,245` | Project 列表渲染/排序/隐藏 |
| 屏幕路由 | `screen` `detailId` `detailSource` `246-250` | list ↔ detail 切换 |
| Base folders 配置 | `baseFoldersOpen` `baseFolderPath` `253-254` | Git Base Folder 维护 |
| Add Project | `addOpen` `addPath` `255,260` | 收录单个仓库 |
| 扫描发现 | `hasScanned` `scanSel` `256-257` | Project discovery 选择 |
| 删除流程 | `deleteId` `deleteAck` `258-259` | 二次确认删除 |
| 三个自定义源 modal | `customDirsOpen/Input` 等 `261-266` | 见 `[[260627-1844-project-自定义源-modal-收敛为单一深组件]]` |

这是 React 版的 **shallow module**：组件的"interface"（它的内部耦合面）巨大——21 个 state + 一堆 handler 全暴露在同一作用域，任何一个 cluster 的 state 都能被其它 cluster 的渲染读到。改"扫描发现"流程，要在 1500 行里找它散落的 `hasScanned`/`scanSel` 和相关 handler；没有 locality，没有任何一块能脱离整页单测。

### deletion test

把"扫描发现"抽成 `useProjectScan` hook 后删掉它 → `hasScanned`/`scanSel`/选择逻辑/scan mutation 会散回 ProjectPage 顶层 ⇒ 复杂度集中，是真 hook。

## What to build

把每个 cluster 抽成 **custom hook**（return 是小 interface，藏住 state + effect + mutation），ProjectPage 退回成「装配 + 路由」的薄壳：

```ts
const scan = useProjectScan();          // { hasScanned, selection, toggle, runScan, confirmSelected }
const add = useAddProject();            // { open, path, setPath, submit }
const baseFolders = useBaseFolders();   // { open, path, list, add, remove }
const deletion = useProjectDeletion();  // 复用既有 lib/query/projectDeletion.ts，补上 deleteId/deleteAck 流程态
```

并把两大视图拆成 presentational 组件（小 props）：

```text
<ProjectListView />    // 列表 + 拖拽 + 右键菜单 + AssetCell（已有）
<ProjectDetailView />  // 详情 + 三个自定义源入口（用 StringListConfigModal）
```

`SortableProjectRow`（141）、`AssetCell`（189）已经是抽出来的小组件，沿用同样的方向继续。

## Suggested shape

- **hook 的 return 即 interface**：调用方只看到 `{ 数据, 动作 }` 几个字段，内部 useState/useEffect/mutateAsync 全藏住——这正是"小 interface + 大行为"的深 module（leverage：一个 `useProjectScan` 可被 list 视图与未来的批量入口复用）。
- **接受依赖、可单测**：hook 内部用既有 `lib/query/*` 数据钩子；逻辑性强的纯函数（如扫描选择的全选/反选、删除二次确认门槛）抽成纯函数便于直接测，不必渲染整页。
- **删除流程已半抽**：`lib/query/projectDeletion.ts` 已存在；本 issue 只把 UI 流程态（`deleteId`/`deleteAck` 二次确认）并进 `useProjectDeletion`，让删除门槛逻辑有单一 locality。
- **先做自定义源 modal**：`[[260627-1844-project-自定义源-modal-收敛为单一深组件]]` 是本拆解的子集且零风险，建议先落地，ProjectPage 立刻少 ~6 个 useState。
- **不为"路由"造抽象**：`screen`/`detailId` 这种 list↔detail 切换保持简单的本地 state 或最小路由，不引入路由库。

## Before / After

```text
BEFORE  ProjectPage (1500 行 / 21 useState)
  list+drag | routing | baseFolders | addProject | scan | delete | customSources
   ↑ 6+ 互不共享的 cluster 摊在一个作用域，无 locality，不可分块单测

AFTER  ProjectPage = 薄装配壳
  useProjectScan / useAddProject / useBaseFolders / useProjectDeletion   ← 小 return，藏行为
  <ProjectListView /> + <ProjectDetailView />                            ← 小 props，纯展示
  <StringListConfigModal />（见姊妹 issue）
```

## Acceptance criteria

- [ ] 扫描发现 / Add Project / Base folders / 删除流程各自收敛为一个 hook，相关 state 不再裸露在 ProjectPage 顶层。
- [ ] ProjectPage 顶层 useState 数量显著下降（目标 ≤ ~8，主要剩路由/装配态）。
- [ ] 列表视图与详情视图拆为独立 presentational 组件，props 为小而明确的接口。
- [ ] 至少扫描选择逻辑与删除确认门槛以纯函数/hook 形式可单测，不依赖整页渲染。
- [ ] 对外行为不变：收录、扫描续认（Stale Project 处置）、隐藏、删除、详情切换均与现状一致。

## Out of scope

- 不改任何 Project IPC 或 react-query key 结构。
- 不引入路由库或全局状态库（Redux/Zustand 等）。
- 不动 SyncPage / ProviderPage（各有独立 issue）。

## Notes

ProjectPage 是前端最大的单文件（1752 行），也是 God-component 最严重的一处。本 issue 与 `[[260627-1844-project-自定义源-modal-收敛为单一深组件]]` 同源，建议把自定义源那刀作为第一个 PR、风险最低，再逐 cluster 抽 hook。与后端 `[[260627-1838-provider-quota-按-provider-垂直切分]]` 是同一类问题（大单元无内部 seam），可一并体现在团队的"深化"标准里。
