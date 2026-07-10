# CodeX 支持 Window Alignment

## Goal

让 CodeX 与 Claude Code 一样支持 Window Alignment：用户可以选择 CodeX 可用模型、配置本地每日首次触发时间，并手动触发一次最小推理请求，以主动锚定 CodeX 的 5 小时额度窗口。

## Background / confirmed facts

- 当前 Window Alignment 已完整支持 Claude Code，但前端在 `src-react/src/components/provider/ProviderPage.tsx` 以 `configId === "claude"` 限制入口、模型查询和保存字段。
- `crates/nexus-core/src/services/provider_trigger.rs` 的 `ProviderTriggerRunner` 目前只有 Claude 实现；不支持的 provider 返回 `supported=false`，启用不支持的 provider 会被后端拒绝。
- CodeX 已有 provider quota 适配器，可从 CodeX 配置目录的 `auth.json` 读取 access token 与可选 account id，并访问 `chatgpt.com/backend-api/wham/usage`。OpenAI Codex 官方源码进一步确认 ChatGPT auth 的 trigger base URL 为 `https://chatgpt.com/backend-api/codex`，模型列表为 `/models`，推理端点为 `/responses`。
- Window Alignment 必须发真实、最小的推理请求；仅调用 quota 查询端点不能假定会重置窗口。动态模型列表优先于硬编码模型 id，触发行为必须保持显式可见且可能产生少量计费。
- 任务涉及 `nexus-core` provider trigger、Tauri command 装配和 React Provider 设置界面，但不需要新增数据库字段；现有 schedule 表和 IPC DTO 已包含 provider-neutral 的 window alignment 字段。

## Requirements

1. CodeX provider trigger 实现复用现有 CodeX `auth.json` 凭据读取方式，支持动态获取可用触发模型，并支持使用用户选择的模型发起最小推理请求。
2. 后端能力查询对 `codex` 返回 `supported=true` 与模型列表；未配置凭据、认证失败、模型/请求失败应沿用现有错误分类和 Window Alignment 状态记录语义。
3. CodeX 的自动调度、5 小时冷却、失败记录、并发合并和手动触发行为与现有 Claude Code 逻辑一致；不改变 Claude Code 行为。
4. 前端在 CodeX 配置中开放 Window Alignment：查询并展示 CodeX 模型，保存 CodeX 的 start time / model，支持手动触发；Claude Code 仍保持现有行为，其他未支持 provider 仍显示 Coming soon。
5. 增加覆盖 CodeX trigger 支持、模型/请求协议或适配器行为、服务层 provider dispatch，以及前端 provider capability 条件的测试；不得依赖真实网络或真实凭据。模型列表过滤不支持 API 的模型，触发器消费完整 SSE 响应并识别流内错误。
6. 保持 provider quota 的 global resource 语义，不把 Window Alignment 配置归因到 Project，也不接管 CodeX 登录或凭据生命周期。

## Acceptance Criteria

- [ ] CodeX 在 `list_provider_trigger_models` 中被识别为 supported，并通过 CodeX 认证上下文返回动态模型列表。
- [ ] CodeX 手动触发和自动调度均使用用户选定模型发送最小推理请求；成功、认证失败、限额/网络失败能写入既有状态字段并在前端可见。
- [ ] CodeX 配置弹层可配置、保存和重新加载 Window Alignment，按钮和模型选择状态正确；Claude Code 与未支持 provider 行为不回归。
- [ ] Rust 与前端相关测试通过，TypeScript strict 检查通过；Windows Rust 测试使用 `pnpm rust:test` 或 SQLite 包装脚本。

## Technical decision

- 采用 CodeX CLI 当前兼容的 ChatGPT Responses 协议：复用 `auth.json` 的 bearer token/account id，使用 `https://chatgpt.com/backend-api/codex/models` 动态获取模型，使用 `https://chatgpt.com/backend-api/codex/responses` 发起最小流式推理请求。详细证据记录在 `research/codex-responses-protocol.md`。

## Out of scope

- 不修改 CodeX quota 快照协议或登录流程。
- 不新增按 provider 的 token 成本估算、自动选最便宜模型或 quota 用尽自动暂停。
- 不扩展 Copilot、OpenCode 或其他 provider 的 Window Alignment。
