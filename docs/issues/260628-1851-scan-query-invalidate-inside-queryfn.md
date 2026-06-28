# scan query 的 queryFn 内 invalidate — 范式债与重构路径

## 问题

为了让 Session / Skill / Prompt 三类 scan 完成后让 `projectKeys.all` 跟着失效，我们在 4 个 query hook 的 `queryFn` 内、`await api.scan()` 之后调了 `queryClient.invalidateQueries({ queryKey: projectKeys.all })`：

- `src-react/src/lib/query/sessions.ts:13` `useLocalSessionsQuery`
- `src-react/src/lib/query/sessions.ts:31` `useCloudSessionsQuery`
- `src-react/src/lib/query/skills.ts:17` `useSkillsQuery`
- `src-react/src/lib/query/prompts.ts:17` `usePromptsQuery`

`queryFn` 里调 `invalidateQueries` 不是 React Query 标准范式。`queryFn` 的契约是"纯数据获取"，副作用应放在 mutation 的 `onSuccess` 里或调用方的 `useEffect` 里。当前实现工作正常，但属于范式债。

## 现状

四个 hook 模式完全对称：

```ts
export function useLocalSessionsQuery() {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: sessionKeys.local,
    queryFn: async () => {
      const sessions = await sessionsApi.scanLocal();
      void queryClient.invalidateQueries({ queryKey: projectKeys.all });
      return sessions;
    },
    enabled: isTauriRuntime(),
  });
}
```

事实：

- 初次 mount 即触发一次 scan + invalidate（无 observer 时 invalidate 是惰性 no-op）。
- 任何对该 query 的 `refetch()` 都会再次触发 invalidate。
- 测试 `tests/component/scanInvalidatesProjects.test.tsx` 已锁住"scan 后 projects 失效"的行为。

## 决策待定

未来若要把这 4 处改成 React Query 标准范式，有三条路径：

### 方案 A：把 scan 改成 mutation，单独保留 list query

- 引入 `useScanLocalSessionsMutation` / `useScanCloudSessionsMutation` / `useScanSkillsMutation` / `useScanPromptsMutation`，每个 mutation `onSuccess` 里 `invalidateQueries(sessions|skills|prompts)` + `invalidateQueries(projects)`。
- 同步新增 `useListLocalSessionsQuery` / 等等，调用后端的 `list_*` 命令（已有：`sessions.rs:108 list_local_sessions`，对应 `list` 在 `api/sessions.ts` 已经存在但前端没用）。
- 调用方从 `useQuery + refetch()` 改为 `useMutation + queryClient.refetchQueries()`。

优点：

- 完全符合 React Query 范式。
- scan 与 list 语义清晰分开：mutation 是副作用，query 是数据。

缺点：

- 改动面大：4 个 hook 重写 + SessionPage / SkillPage / PromptPage / ProjectDetailView 的 refresh 按钮调用方都要改。
- 后端 `list_*` 与 `scan_*` 双接口要保证语义清晰（list 是查已索引、scan 是落盘重新建索引），要在 doc 注释里说明。

### 方案 B：保留 queryFn 副作用，但提到一个 helper 集中管理

- 新增一个 `invalidateProjectsAfterScan(queryClient, promise)` 函数，让 4 个 hook 共用。
- 不改范式债，但让副作用只在一处。

优点：

- 改动小，5 分钟搞定。
- 如果未来重构成 mutation，只需要删 helper，调用方不动。

缺点：

- 没解决范式债本身，只是把"债的位置"集中了。
- `queryFn` 内仍有副作用。

### 方案 C：维持现状，记账即可

- 当前实现工作正确、有测试覆盖、未来重构时一目了然。
- 本 issue 存在就是给未来重构留指针。

优点：

- 零工作量。
- 改动少 = churn 风险低，符合 CLAUDE.md "Simplicity First"。

缺点：

- Reviewer 看到 `queryFn` 内调 `invalidateQueries` 会皱眉。
- 没消解债务。

## 建议

**维持现状（方案 C）**，理由：

- 当前代码改动是为了让 Project 列表的 `p.sessions` / `p.skills` / `p.prompts` 数字跟着 scan 同步，是一次性、低风险的功能修复。
- "把所有副作用都收进 mutation 的 onSuccess" 是值得追求的工程洁癖，但本工作区 `Session` / `Skill` / `Prompt` 三个 hook 当前就是 `useQuery + refetch()` 范式，改成 mutation 是一次独立的范式统一工作，应该单独评估、单独排期，不能挂在"修 Project 数字不刷新"这条线路上。
- `invalidateQueries` 在 `queryFn` 内虽然不标准，但语义清晰（"scan 后让 projects 失效"），配合命名良好的变量名（`invalidateQueries` 不返回 Promise，void 显式丢弃）不至于产生微妙 bug。

## 未来实现约定

如果未来决定走方案 A（mutation 化）：

- 后端：`list_local_sessions` / `list_cloud_sessions` / `list_skills` / `list_prompts` 四个 `list_*` 命令都已存在（`services/sessions.rs:108` / `:112`、`services/skills.rs:118`、`services/prompts.rs:134`）；不需要新增。
- 前端：`lib/api/{sessions,skills,prompts}.ts` 已经各自 export 了 `list` / `listLocal` / `listCloud`（`api/sessions.ts:5` / `:9`、`api/skills.ts:11`、`api/prompts.ts:11`），只是当前没有 query hook 调用它们；把对应 `useXxxQuery` 改为 `queryFn: api.list` 即可。
- mutation：新增 `useScanLocalSessionsMutation`，`mutationFn: sessionsApi.scanLocal`，`onSuccess` 同时 invalidate `sessionKeys.local` 与 `projectKeys.all`。
- 调用方迁移：所有 `query.refetch()` 的地方改成 `await scanMutation.mutateAsync()` + `await queryClient.refetchQueries({ queryKey })`。

工作量估计：4 个 hook + 4 个 mutation + 5-6 个调用点 + 2-3 个新测试。属于半天到一天的小重构。

## 验收标准

- [ ] 本 issue 不阻塞当前 Session/Skill/Prompt → Project 数字联动功能上线。
- [ ] 未来如果重构到方案 A，要保证 `scanInvalidatesProjects.test.tsx` 在新结构下继续通过（可改名/重写但语义保留）。
- [ ] 重构时不要借机改"scan 是否同时 invalidate skills/prompts/sessions 互查"的现有行为——那是另一个独立的设计问题。

## 备注

- 真正需要范式统一的时机是引入"scan 失败时不要 invalidate"或"scan 部分成功时只 invalidate 成功那部分"这类带条件的副作用，那时再走方案 A 就自然了。
- 当前实现的"白嫖副作用"：ProjectDetailView mount 时订阅 `useLocalSessionsQuery`/`useSkillsQuery`/`usePromptsQuery`，会自动触发一次 scan + projects invalidate——这恰好让"打开 Project 详情 → Project 列表数字保持新鲜"成立，是个意外的好副作用，但**不应当作设计意图**，重构时要重新评估。