# Frontend Type Safety

## 适用范围

适用于 `src-react/src/types/`、`src-react/src/config/`、`src-react/src/lib/api/` 和组件 props 类型。

## 类型来源

- 共享领域类型集中在 `types/index.ts`，字段命名与 Tauri IPC payload / `CONTEXT.md` glossary 对齐。
- Agent 名称来自 `config/agents.ts` 导出的 capability 类型；`types/index.ts` 用 `AgentName` 约束 `Cells`。
- API 层每个函数声明返回类型，例如 `lib/api/sync.ts` 返回 `TaskGroup[]`、`Task` 等，组件不处理 `unknown` payload。

## 命名与领域术语

- UI 展示和类型注释应使用 canonical domain names：`Agent`、`Provider`、`Project`、`Skill`、`Prompt`、`Session`、`Distribution`、`Cloud`。
- `AgentName` 展示值必须使用 `CONTEXT.md` 确认的完整 canonical 名称，如 `Generic Agent`、`Claude Code`、`CodeX`、`Copilot`、`OpenCode`；短 ID 仅用于实现层 provider id 或配置 key。若代码中出现尚未写入 `CONTEXT.md` 的 capability entry，新增规范时应先澄清领域身份，不要直接扩充 canonical list。
- `LocationType` 使用 `Local` / `Cloud`；主内容 UI 不把 `WebDAV` 当作用户手选 location type。

## Runtime validation 现状

- 项目当前未引入 Zod / React Hook Form；表单校验以受控组件 + 纯规则 module 为主。
- 后端是最终校验层；前端类型不能替代 `nexus-core` 的 service validation。

## 常见错误 / anti-pattern

- 不要用 `any` 绕过 IPC payload 类型；若后端新增字段，先扩展 `types/index.ts` 或领域局部类型。
- 不要把实现层短 ID 当展示类型，例如把 `claude` 显示成 Agent 名称。
- 不要给 `Project` 身份绑定本地 path；`CONTEXT.md` 明确 `Project Key` 是稳定身份，`Project Path` 可变。

## 验证

- `src-react/package.json` 提供 `typecheck`：`tsc --noEmit`。
- 测试编译使用 `tsconfig.test.json`，纯 module 需保持 Node 可编译，避免依赖浏览器/Tauri runtime。
