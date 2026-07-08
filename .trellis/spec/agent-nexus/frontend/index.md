# Agent Nexus 前端规范

本目录描述 `src-react/` 的真实开发约定。前端是 React 18 + Vite + TypeScript strict + Tailwind CSS，运行在 Tauri 桌面壳内。

## 规范清单

| Guide | 用途 |
|-------|------|
| [Directory Structure](./directory-structure.md) | `src-react/src` 的分层、领域目录和文件放置规则 |
| [Component Guidelines](./component-guidelines.md) | 页面组件、领域组件、UI primitive 与样式模式 |
| [Hook Guidelines](./hook-guidelines.md) | React hook、React Query hook、Tauri IPC hook 的组织方式 |
| [State Management](./state-management.md) | 本地 UI state、NavContext、React Query server state 的边界 |
| [Quality Guidelines](./quality-guidelines.md) | 测试、验证命令、项目特有 anti-pattern |
| [Type Safety](./type-safety.md) | TypeScript 类型组织、领域术语和 IPC payload 约定 |

## 必读背景

- `CONTEXT.md`：领域术语和 `_Avoid_` 语义；UI 展示必须使用 `Agent` canonical names，例如 `Claude Code` / `OpenCode`，不能用实现层短 ID 替代。
- `GOTCHAS.md`：UI 可以超前后端能力，但不要删除或隐藏相关 UI 元素；窗口最小尺寸已由 `src-tauri/tauri.conf.json` 限制，不需要小屏优先设计。
- `docs/design/Architecture Design.md`：前端采用 typed API layer + React Query 单一真相源 + 自写 themed primitive。

## 通用验证

- 类型检查 / 构建：`cd src-react && pnpm typecheck` 或根目录脚本对应命令。
- 单元测试：`cd src-react && pnpm test:unit`。
- 组件测试：`cd src-react && pnpm test:component`。
