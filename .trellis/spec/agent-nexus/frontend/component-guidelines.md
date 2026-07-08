# Frontend Component Guidelines

## 适用范围

适用于 `src-react/src/components/` 的页面组件、领域组件和 UI primitive。

## 组件层级

- `components/shell/` 负责全局框架；`App.tsx` 根据 `View` 渲染 `ProviderPage`、`ProjectPage`、`SkillPage` 等页面。
- `components/<domain>/` 负责领域页面与领域内组件，例如 `components/provider/ProviderPage.tsx`、`components/skill/SkillRow.tsx`。
- `components/ui/` 是项目自写 themed primitive，例如 `button.tsx`、`modal.tsx`、`select.tsx`；架构文档明确当前未使用 shadcn CLI / Radix。

## Props 与组合模式

- 小型 UI primitive 使用 `forwardRef` + 原生 HTML props + CVA variants。参考 `components/ui/button.tsx` 的 `ButtonProps`、`IconButtonProps`。
- 领域组件优先接收领域类型或明确的 handler，不把 IPC command name、query key 等底层细节透传到叶子组件。
- Modal / form 组件保持受控输入；项目当前未引入 React Hook Form + Zod，架构文档记录表单仍为受控组件。

## 样式模式

- 使用 Tailwind utility + `cn` 合并 class；variant 组件使用 `class-variance-authority`。
- 使用项目 token（如 `nexus-accent`、`nexus-card`、`nexus-border2`），不要随意引入另一套设计系统。
- `GOTCHAS.md` 已说明 `tauri.conf.json` 限制 `minWidth: 1100` / `minHeight: 720`，新增 UI 不需要为很小屏幕牺牲桌面信息密度。

## UI 能力超前后端时

`GOTCHAS.md` 明确允许 UI 超前；如果后端暂未实现，交互可以 toast/无反应，但不要删除或隐藏设计中已有 UI 元素。新增代码应让缺失能力以温和错误呈现，而不是移除入口。

## 常见错误 / anti-pattern

- 不要引入 shadcn/Radix 组件替换现有 primitive，除非任务明确要求复杂可访问性能力并同步设计取舍。
- 不要把 Provider 与 Agent 混为一谈；`OpenCode Go` 是 `Provider quota` 入口，不是 `Agent`。
- 不要在页面组件里混入可测试的纯业务规则；像 `components/sync/taskRules.ts` 一样抽成 dependency-free module 并配测试。
