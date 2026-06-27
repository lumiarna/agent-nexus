# provider_quota 按 Provider 垂直切分（深化 per-provider locality）

> 架构深化 issue（由 improve-codebase-architecture 探索得出）。推荐强度：**Strong**。
> 词汇遵循 `CONTEXT.md`（Provider / Provider Quota）与 codebase-design（module / interface / depth / seam / adapter / locality）。

## 问题

`crates/nexus-core/src/services/provider_quota.rs` 单文件 **3120 行**，承载 8 个 `Provider`（Claude Code / Codex / Copilot / OpenCode Go / MiniMax / DeepSeek / OpenRouter / OpenCode custom）的全部 quota 观测逻辑。

`ProviderQuotaService` 的 **interface 很小**（`new` / `get_provider_quota(provider_id)` / `list_opencode_custom_providers`），整体看是个 deep module，这一点是对的、不要破坏。真正的 friction 在 module **内部没有 per-provider seam**：理解或修改任何一个 Provider，都要在文件里来回跳 6 个互不相邻的区域。以 Claude Code 为例：

| 关注点 | 当前位置（行号） |
| --- | --- |
| 常量（URL / client_id / keychain service） | 27–31 |
| 响应结构体 `ClaudeCodeUsageResponse` 等 | 153–162 |
| Adapter `ClaudeCodeQuotaAdapter::quota_snapshot` | 700–782 |
| 纯函数 `claude_code_quota_from_usage_response` | 1064 |
| `derive_claude_code_snapshot` | 1367 |
| `fetch_claude_code_usage` / OAuth refresh / keychain 读写 | 2125–2336 |

每个 Provider 都是这样被「摊平」铺在 3000 行里。已有的 `ProviderQuotaAdapter` trait（451 行）是个好 seam，**但它只包住了 `quota()` 一个方法**；一个 Provider 的凭据读取、HTTP 拉取、响应解析、snapshot 派生、status 构造全部漏在 trait 外面，作为自由函数散落全文件。

### deletion test

- 现状：要删掉 OpenRouter 支持，得在常量、响应结构体、`OpenRouterQuotaAdapter`(走 `ConfiguredProviderQuotaAdapter`)、`openrouter_credits_quota_from_usage_response`、`fetch_openrouter_credits` 等 5+ 处分别动刀，漏一处就留死代码。
- 期望：删掉 `providers/openrouter.rs` 一个文件，OpenRouter 支持干净消失，注册表少一行。

复杂度「集中」而非「搬家」——这正是要找的信号。

## What to build

把每个 Provider 收敛成一个**自包含子模块**，藏在现有 `ProviderQuotaAdapter` seam 之后（必要时把 trait 加宽到覆盖 credential + fetch + parse + derive，让一个 Provider 的全部知识都在 trait 实现内闭合）。

目标目录形态（示意，非强制）：

```text
services/provider_quota/
  mod.rs                 // ProviderQuotaService（小 interface）+ adapter 注册表 + 共享类型
  shared.rs              // 跨 provider 真正复用的深逻辑
  providers/
    claude_code.rs       // 常量 + 响应结构体 + 凭据读取 + fetch + parse + derive + status
    codex.rs
    copilot.rs
    opencode_go.rs
    configured.rs        // MiniMax / DeepSeek / OpenRouter（headers-driven gateway 那一类）
    opencode_custom.rs
```

`mod.rs` 保留并维护：
- `ProviderQuotaService` 的小 interface（不变）。
- `provider_quota_adapters()` 注册表（当前在 530 行，返回 `[&dyn ProviderQuotaAdapter; 5]`）——这是新增/删除 Provider 的**唯一**入口。
- 共享类型 `ProviderQuotaSnapshot` / `ProviderQuotaWindow` / `ProviderQuotaStatus`。

## Suggested shape

### 什么留在 shared，什么下沉到 provider

**下沉**（每个 provider 私有，今天散在全文件）：
- 该 provider 的 URL/client_id/keychain 常量
- 响应 `Deserialize` 结构体
- `fetch_*` async 拉取
- `*_quota_from_*` 纯解析函数
- `derive_*_snapshot` 与 `*_status` 构造

**保留在 shared**（确认是「两个以上 provider 真的复用」才留，符合 "one adapter = hypothetical seam, two = real"）：
- `quota_window` / `gateway_quota_window`（1905 / 2008）
- `percent_to_u8` / `shortest_percent_window_used` / `quota_window_kind_rank`
- `http_client`（2095）、`provider_quota_log_context`、`OutboundRequestLogger` 接线
- `*_to_iso` 时间换算族（`unix_seconds_to_iso` / `reset_seconds_to_iso` / `next_natural_month_reset_at` 等）
- `ProviderCredentialSource` / `ProviderUsageTransport` 两个既有 seam（被 provider_trigger 复用，见 `[[260627-1838-claude-access-token-生命周期-共享深模块]]`，**不要动它们的签名**）

判定规则：一个 helper 只被单个 provider 调用 → 下沉到该 provider 文件；被两个及以上调用 → 留 shared。

### 测试面：interface 即测试面

大量 `*_quota_from_*` 纯函数已被 `tests/provider_quota.rs`（788 行）与文件内 `#[cfg(test)]` 直接单测。这是该 module 当前最值钱的 **test surface**，迁移后：
- 这些纯函数必须保持 `pub`（或 `pub(crate)`）从子模块 re-export，**测试不改一行断言**。
- 验证迁移正确性的标准就是 `tests/provider_quota.rs` 全绿且零改动。

### configured / gateway 一类

MiniMax / DeepSeek / OpenRouter 走 `ConfiguredProviderQuotaAdapter` + `llm_gateway_quota_from_headers`（1921）这条 headers-driven 路径，与 `[[260624-1453-llm-api-gateway-特殊逻辑]]` 记录的端点约定相关。它们共享度高，建议合并进 `providers/configured.rs` 一个文件，而不是每个拆一份——避免为「假 seam」造文件。

## Before / After

```text
BEFORE  ── provider_quota.rs (3120 行) ────────────────────
  小 interface ✓        ProviderQuotaAdapter seam（只包 quota()）
  ─────────────────────────────────────────────
  [常量×8][响应×20][fetch×9][parse×8][derive×5][status×5][keychain][shared helper]
   ↑ 一个 provider 的知识被横切摊开，无 per-provider locality

AFTER  ── provider_quota/ ─────────────────────────────────
  mod.rs            小 interface ✓ + 注册表（增删 provider 唯一入口）
  shared.rs         真复用的深 helper（window/iso/http/log）
  providers/claude_code.rs   ← 一个 provider 的全部知识闭合在一个文件
  providers/codex.rs ...     ← deletion test: 删文件即删能力
```

## Acceptance criteria

- [ ] `ProviderQuotaService` 的公开 interface（方法签名）保持不变。
- [ ] 每个 Provider 的常量 / 响应结构体 / fetch / parse / derive / status 收敛到该 Provider 自己的子模块文件。
- [ ] 新增或删除一个 Provider 时，`mod.rs` 的 adapter 注册表是唯一需要改的「接线」位置。
- [ ] `ProviderCredentialSource` / `ProviderUsageTransport` 两个被 provider_trigger 复用的 seam 签名不变。
- [ ] `tests/provider_quota.rs` 与文件内单测**零断言改动**全绿（仅允许调整 `use` 路径）。
- [ ] shared 模块内不残留只被单个 provider 调用的 helper。

## Out of scope

- 不改任何 Provider 的 quota 拉取语义、URL、解析口径。
- 不动 `provider_trigger.rs`（另见 `[[260627-1838-claude-access-token-生命周期-共享深模块]]`）。
- 不引入新的 Provider，不做 quota 缓存/调度变更。
- 不改 `ProviderQuotaSnapshot` 的 serde 形态（前端契约不变）。

## Notes

这是纯 locality 重构，**对外行为零变化**，风险点只在「迁移时把某个 helper 的可见性/路径搞错」，由零改动测试套兜住。建议分 Provider 增量提交（一个 PR 搬一个 provider），每次保持测试全绿，避免一次性 3000 行大挪移。
