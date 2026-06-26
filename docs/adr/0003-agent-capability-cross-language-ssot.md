# Agent Capability Surface 跨语言 SSOT

## Status

Accepted.

## Context

`Agent Capability Surface` 同时被后端资产发现、Agent Matrix、Provider 展示、Settings 展示和前端离线 preview 使用。当前 Rust 后端已有 `crates/nexus-core/src/services/agent_capabilities.rs`，前端也有 `src-react/src/config/agents.ts` 作为浏览器 preview 与类型字面量来源。

这两份定义如果长期都被视为真相源，会在 canonical order、目录、Prompt 文件、颜色和 Provider credential hint 上产生漂移。Prompt 和 Provider 已经开始在运行时消费后端能力面，因此需要先明确跨语言事实归属，再决定是否上生成链。

## Decision

运行时 source of truth 采用 **Rust 后端 `Agent Capability Surface`，通过 IPC 暴露给前端消费**。

TypeScript 的 `src-react/src/config/agents.ts` 暂时保留，但只作为离线浏览器 preview fallback 和前端字面量类型来源，不作为桌面运行时的产品真相源。后续通过独立 AFK issue 收敛前端对 fallback 的依赖，并补充 drift 防护。

## Source Of Truth

| Fact | Runtime source of truth | Preview / type fallback |
|---|---|---|
| canonical agent order | Rust `agent_capability_surfaces()` | `src-react/src/config/agents.ts` |
| config roots | Rust `AgentCapabilitySurface.config_dir` | `src-react/src/config/agents.ts` |
| Skill global/project dirs | Rust `SkillSurface` | `src-react/src/config/agents.ts` |
| Prompt global/project files | Rust `PromptSurface` | `src-react/src/config/agents.ts` |
| colors / abbreviations | Rust `AgentCapabilitySurface.color` / `abbr` | `src-react/src/config/agents.ts` |
| Provider identity / credential hints | Rust `ProviderSurface` | `src-react/src/config/agents.ts` |

Provider quota polling details、Provider connection params、OpenCode custom provider discovery 不属于 `Agent Capability Surface` 的真相范围，继续留在 Provider/Provider quota 边界内。

## Considered Options

### Option 1: Keep Parallel Rust / TypeScript Modules With Tests

优点：

- 构建复杂度最低，不引入 build script、schema 或生成产物。
- 前端离线 preview 最顺畅，Vite 不依赖 Tauri runtime。
- 两侧类型都保持本地语言原生表达。

缺点：

- canonical order、Prompt 文件、Provider hint 很容易 drift。
- 测试只能发现漂移，不能从结构上消除漂移。
- 后端 Prompt/Provider 已经消费 Rust 能力面，继续把 TS 当同级真相源会弱化边界。

### Option 2: Generate Rust / TypeScript From A Shared Data File

优点：

- 真正只有一份静态数据源。
- 类型可以由生成代码承担，前后端字段更容易保持一致。
- 离线 preview 可以直接 import 生成出的 TS 文件。

缺点：

- 需要引入 schema、生成脚本、构建顺序和产物提交策略。
- 当前能力面事实仍在演化，过早生成会把不稳定形状写进工具链。
- 打包和 CI 需要确认生成产物在 Rust、Tauri、Vite 三条链路中一致可用。

### Option 3: Expose Backend Capability Surface Through IPC

优点：

- 运行时事实天然由后端控制，Prompt/Provider/Settings 使用同一份能力面。
- 构建复杂度低于生成方案，不需要跨语言 codegen。
- 与现有 `list_agent_capabilities` 命令和 React Query 接入一致。

缺点：

- 纯浏览器 preview 仍需要 fallback 数据。
- TypeScript 编译期无法直接证明后端返回字段与前端类型一致，只能靠接口类型和测试。
- 若未来更多前端纯函数需要能力面字面量，仍需 drift 防护或生成方案。

## Consequences

- 后端新增或修改 agent 能力面时，运行时页面应通过 IPC 自动获得变更。
- 前端 fallback 不得用于桌面运行时覆盖后端事实；它只服务非 Tauri preview。
- 如果后续发现 fallback drift 成本持续升高，再升级到共享数据生成；当前不为此票据引入 code generation。
- Prompt 和 Provider 的后端/运行时行为应继续从能力面派生，不再在局部模块复制 agent 顺序或 provider credential hint。

## Follow-Up

后续 AFK issue: `docs/issues/260626-1913-agent-capability-frontend-ipc-ssot-hardening.md`。
