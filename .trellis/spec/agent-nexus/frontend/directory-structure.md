# Frontend Directory Structure

## 适用范围

适用于 `src-react/src/` 下的 React / TypeScript 前端代码。

## 本项目结构

```text
src-react/src/
├── App.tsx                         # View 联合 + NavContext 驱动页面切换
├── components/
│   ├── ui/                         # 自写 themed primitive，不使用 shadcn CLI
│   ├── shell/                      # AppShell / 页面框架
│   ├── provider|project|skill|prompt|session|sync|settings/
│   │                                # 按领域组织页面、组件与领域内纯 helper
├── config/                         # 前端静态配置，如 Agent 展示信息
├── lib/
│   ├── api/                        # typed Tauri invoke 封装，组件不得直接 invoke
│   ├── query/                      # React Query hooks 与 query keys
│   ├── nav.tsx / runtime.ts / tokens.ts / utils.ts
├── types/                          # 与 IPC payload 对齐的共享领域类型
└── tests/                          # Node/Vitest 测试
```

参考文件：`src-react/src/App.tsx`、`src-react/src/lib/api/tauri.ts`、`src-react/src/lib/query/projects.ts`、`src-react/src/types/index.ts`。

## 放置规则

- 新页面或复杂功能按领域放入 `components/<domain>/`，例如 `components/project/ProjectPage.tsx`、`components/sync/SyncPage.tsx`。
- 跨领域复用的视觉原语放入 `components/ui/`；领域特有 helper 留在领域目录，例如 `components/sync/taskRules.ts`、`components/project/stringListEdit.ts`。
- 后端 IPC 访问只放在 `lib/api/<domain>.ts`；React Query 封装只放在 `lib/query/<domain>.ts`。
- 跨页面导航不引入 URL router；遵循 `App.tsx` + `lib/nav.tsx` 的 `View` union 和 `NavContext`。

## 常见错误 / anti-pattern

- 不要在组件中直接调用 `@tauri-apps/api/core.invoke`；使用 `lib/api/tauri.ts` 的 `invokeCommand` 和领域 API 文件。
- 不要把服务端数据复制成页面级 `useState` 镜像；服务端状态属于 `lib/query/`。
- 不要把 `WebDAV` 作为主内容 UI 的 location 文案；`CONTEXT.md` 要求 UI 层优先使用 `Cloud`。
- 不要用实现层 agent id（如 `claude`、`opencode`）作为 UI 展示名；展示名使用 canonical `Agent` 名称。
