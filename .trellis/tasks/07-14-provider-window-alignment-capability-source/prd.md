# 统一 Provider Window Alignment 能力真相源

## Goal

深化现有 Provider Trigger module 的 capability interface，使其成为 Window Alignment 支持情况与触发材料的唯一真相源，删除前端必须预知后端能力的浅重复。

**推荐强度：Strong**

## Evidence

- 后端 `ProviderTriggerService` 已隐藏调度、重试、并发抑制和触发实现，并有 Claude Code、CodeX 等真实 adapter：`crates/nexus-core/src/services/provider_trigger.rs:124-395,570-682`。
- 前端 `src-react/src/components/provider/windowAlignmentSupport.ts` 硬编码 `claude` / `codex`。
- `ProviderPage.tsx` 与 `lib/query/providers.ts` 在查询后端 capability 前先依赖这份硬编码名单，形成循环 interface。
- `086a76c` 新增 CodeX 时必须同时修改后端 adapter 与前端名单；`3af0a3e` 随后再次修正模型映射。

## Requirements

1. Provider Trigger capability 是唯一真相源；调用者不得先靠硬编码名单决定是否查询该能力。
2. 新增支持 Window Alignment 的 Provider adapter 时，不应再同步维护前端 provider ID 名单。
3. 保持现有 Claude Code 与 CodeX 调度、模型选择、手动触发、重试和并发抑制行为。
4. 使用现有多个 adapter 形成的真实 seam，不引入平行 capability registry。
5. capability 测试不得通过真实网络失败来间接证明支持。
6. 实现前比较 capability 的现有 interface 深化方案，本任务不预定最终 interface。

## Acceptance Criteria

- [ ] 删除 `supportsWindowAlignment` 硬编码 module，或使其不再持有 Provider ID 真相。
- [ ] Provider 页面完全根据 Provider Trigger interface 返回值呈现支持状态、模型与错误。
- [ ] 新增一个测试 adapter 的支持能力时，无需修改前端名单即可被正确呈现。
- [ ] capability、模型列表和触发可通过稳定 fake adapter 测试，不触发真实网络。
- [ ] Claude Code 与 CodeX 的现有行为和 canonical 显示名保持兼容。
- [ ] deletion test 复核表明浅前端名单可删除，而深 Provider Trigger module 承担所有能力复杂度。

## Out of Scope

- 修改 ADR-0002 的 DeepSeek CloudFront 决策。
- 重写 Provider quota module。
- 新增没有实际实现的 Provider adapter。
