# Frontend State Management

## 状态分类

- **Server state**：后端 IPC 返回的数据，统一由 React Query 管理。参考 `lib/query/projects.ts`、`lib/query/sync.ts`。
- **Navigation state**：桌面应用不使用 URL router；`App.tsx` 维护 `view` 与 `projectId`，通过 `NavContext` 分发。
- **Local UI state**：表单输入、modal 开关、临时选择等留在组件或领域 hook 内。
- **Pure derived state / rules**：放入可测试纯 module，例如 `components/sync/taskRules.ts`。

## Server state 规则

- 组件读取后端数据必须通过 `useQuery` / `useMutation` 封装，不直接维护“后端数据镜像”。
- Mutation 后必须考虑跨领域失效：例如 Project 的 `extraPromptFiles` 会影响 Prompt scan，`sessionsDir` 会影响 Local Session 与 Session Backup。
- 当后端返回的是权威列表或对象，可以直接 `setQueryData`；当影响扫描结果或其它领域聚合时，使用 `invalidateQueries`。

## Navigation state 规则

- 只能使用 `View` union 中的页面值：`provider`、`project`、`skill`、`prompt`、`session`、`sync`、`settings`。
- 从 Session 跳转 Project detail 这种 deep-link 使用 `NavContext.go("project", { projectId })` 风格，不引入 URL path。

## Sync 表单状态

- 表单可以表达 “1 source → N targets” 的输入便利，但提交前必须经 `expandFormTask` 拆成领域模型 `1 source → 1 target`。
- `Symlink` / `Junction` 只适用于 Local→Local；schedule 只适用于 `Copy`。前端规则只做 UX，后端 `prepare_task` 仍是事务级真相源。

## 常见错误 / anti-pattern

- 不要新增 Zustand/Jotai 等全局状态库；架构文档明确当前不引入。
- 不要让 `View` 之外的字符串在组件间传递作为页面身份。
- 不要把可由 `CONTEXT.md` 推导的领域状态做成更细 UI 状态，例如 `Project Status` 只展示 `active` / `stale` / `hidden`，不要把 moved/renamed 升级为一级状态。
