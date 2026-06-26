# Agent Capability Surface 前端 IPC SSOT 收敛

## What to build

在 ADR 0003 的决策基础上，收敛前端对 `src-react/src/config/agents.ts` 的运行时依赖：桌面运行时应把后端 `list_agent_capabilities` 返回的能力面作为 Agent 展示、Provider 展示和 Settings 展示的事实源；TypeScript agent 定义只保留为离线 browser preview fallback 和类型辅助。

## Acceptance criteria

- [ ] 前端运行时 Agent order、abbr、color、Skill dirs、Prompt files、Provider credential hints 都来自 `list_agent_capabilities`。
- [ ] `src-react/src/config/agents.ts` 被明确命名或注释为 preview fallback，不再被运行时路径当作同级真相源。
- [ ] Agent Matrix 组件可以消费后端能力面派生的顺序，同时 browser preview 仍能渲染。
- [ ] Provider 页和 Settings 页在 Tauri runtime 下不读取 fallback 覆盖后端事实。
- [ ] 增加 drift 防护测试或快照，证明 fallback 与后端能力面的字段差异会被显式发现。
- [ ] 不引入 cross-language code generation；若测试证明 drift 维护成本过高，再单独开生成方案 issue。

## Blocked by

- `docs/adr/0003-agent-capability-cross-language-ssot.md`

## Notes

保持这是前端消费边界收敛，不要把 Provider quota polling、OpenCode custom provider discovery 或 Prompt project/global scope 决策混进来。
