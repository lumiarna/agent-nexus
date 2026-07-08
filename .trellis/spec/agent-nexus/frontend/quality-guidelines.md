# Frontend Quality Guidelines

## 本地质量标准

- 前端代码必须通过 TypeScript strict 检查；脚本见 `src-react/package.json` 的 `typecheck` / `build`。
- 纯业务规则需要单元测试；参考 `src-react/tests/taskRules.test.ts`、`src-react/tests/stringListEdit.test.ts`、`src-react/tests/pathDisplay.test.ts`。
- 组件行为使用 Vitest + Testing Library；参考 `src-react/tests/component/scanInvalidatesProjects.test.tsx`、`src-react/tests/component/connectionForms.test.tsx`。

## 必须遵守的项目规则

- UI 可以领先后端，但不要删除或隐藏设计中已有入口；依据 `GOTCHAS.md`。
- 前端 location 文案优先 `Cloud`，不要在主内容界面暴露实现层 `WebDAV` 概念；依据 `CONTEXT.md`。
- Agent 展示顺序和名称跟随 capability surface / canonical names，不按 provider、账号或短 ID 临时排序。
- 复用现有 `components/ui/` primitive 和 `lib/query/` / `lib/api/` 模式，不引入重复封装。

## 验证命令

在 `src-react/` 下运行：

```bash
pnpm typecheck
pnpm test:unit
pnpm test:component
```

如果任务只改纯 TypeScript module，至少运行对应单元测试；如果改页面交互，优先补充或运行相关 component test。

## 常见错误 / anti-pattern

- 直接在组件里调用 Tauri `invoke`。
- 用 `useState` 保存后端列表并手工同步 React Query cache。
- 引入小屏优先布局导致桌面主视图信息密度下降；当前 Tauri 窗口已有最小尺寸约束。
- 把 `Provider` quota 语义 project-level 归因；`CONTEXT.md` 明确 `Provider` 是 global resource。
