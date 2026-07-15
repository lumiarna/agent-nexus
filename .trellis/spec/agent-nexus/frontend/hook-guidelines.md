# Frontend Hook Guidelines

## 适用范围

适用于 `src-react/src/lib/query/` 与 `components/**/use*.ts` 中的 React hooks。

## React Query hooks

- 所有来自后端的数据通过 TanStack React Query 管理；`docs/design/Architecture Design.md` 将其定义为 server state 单一真相源。
- 每个领域文件导出 query keys 与 hooks，例如 `lib/query/projects.ts` 的 `projectKeys`、`useProjectsQuery`、`useRecordProjectMutation`。
- Mutation 成功后按影响范围更新缓存：能确定返回完整对象时用 `setQueryData`，跨领域派生数据变化时用 `invalidateQueries`。参考 `useSetProjectSessionsDirMutation` 同时失效 `sessionKeys.local` 与 `syncKeys.sessionBackups`。
- 严格区分只读 list query 与会改写状态的 scan：例如 `useSkillsQuery` 只能调用 `list_skills`；会扫描文件系统、获取 mutation lock、替换 source 或 reconcile Distribution 的 `scan_skills` 必须放在显式 Refresh mutation。否则窗口聚焦 refetch 也会产生写入副作用。

## API hook 与 IPC 边界

- Hook 不直接调用 Tauri `invoke`；先经过 `lib/api/<domain>.ts`，再由 query hook 调用。
- 非 Tauri 环境通过 `lib/api/tauri.ts` 的 `invokeCommand` 抛出统一错误：`Agent Nexus desktop runtime is required for this action.`

## 领域内自定义 hooks

- 只在领域内复用的状态逻辑放在对应目录，例如 `components/provider/useTraySync.ts`、`components/project/useProjectScan.ts`。
- 纯规则不要写成 hook；`components/sync/taskRules.ts` 是普通 pure module，可被 Node 测试直接覆盖。

## 常见错误 / anti-pattern

- 不要让多个 hook 维护同一份服务端数据的本地副本。
- 不要在 mutation 后只更新当前页面而忘记跨页面缓存，例如 Project 配置会影响 Skill / Prompt / Session / Sync。
- 不要把 `useEffect` 当作数据获取默认方案；后端数据读取应优先是 `useQuery`。
- 不要把 scan/reconcile command 当普通 queryFn；React Query 自动 refetch 只能安全调用只读 interface。

## 验证

- Query 行为已有专门测试，例如 `src-react/tests/queryClient.test.ts`、`src-react/tests/sessionBackupRecords.test.ts`。
- 纯 hook-adjacent 逻辑可参考 `src-react/tests/taskRules.test.ts`。 
