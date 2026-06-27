# Claude 访问令牌生命周期收敛为共享深模块

> 架构深化 issue（由 improve-codebase-architecture 探索得出）。推荐强度：**Worth exploring**（偏 Strong，因存在逐字重复）。
> 词汇遵循 `CONTEXT.md`（Provider / Claude Code）与 codebase-design（module / interface / depth / seam / locality）。

## 问题

「拿到一个可用的 Claude access token」这条逻辑在 **两个模块里各实现了一遍**，且包含逐字相同的报错文案。

**A. provider_quota** — `ClaudeCodeQuotaAdapter::quota_snapshot`（`provider_quota.rs:726-782`）：
1. 读凭据 `claude_code_credentials`
2. scope 校验：缺 `user:profile` → 失败
3. `is_token_expiring_soon` → `refresh_and_persist_result` 刷新
4. 调 usage；遇 `AuthRequired` → `refresh_and_persist` 再刷新后重试

**B. provider_trigger** — `ClaudeCodeTriggerRunner::claude_access_token`（`provider_trigger.rs:533-568`）+ `list_models`/`trigger`（590-628）内联的重试：
1. 读凭据 `claude_code_credentials`
2. scope 校验：缺 `user:profile` → 失败
3. `is_token_expiring_soon` → `refresh_claude_code_credentials` 刷新
4. 调 models/trigger；遇 auth error → `refresh_or_auth_error` 再刷新后重试

两边是**同一套 4 步生命周期**，连第 2 步的文案都逐字一致：

```text
"Claude OAuth token missing 'user:profile' scope. Run 'claude setup-token'."
```

> 注：底层原语已经共享得不错——`LocalCredentialSource`（读凭据）、`HttpUsageTransport::refresh_claude_code_credentials`（刷新）、`is_token_expiring_soon`（判过期）都由 provider_trigger 直接 `use` provider_quota 的实现。**真正没收敛的是「编排」**：scope-gate + expiry-refresh + auth-retry 这层 orchestration 在两处各写一遍。

### deletion test

把这层编排抽成一个深 module 后删掉它 → scope 校验文案、刷新时机、401 重试策略会同时在 quota 与 trigger 两处重现 ⇒ 它在挣自己的钱，是真 module，不是 pass-through。

## What to build

一个 Claude 凭据编排的**深 module**，interface 小、行为多。两个候选形态：

**形态 1：`acquire` + `with_auth_retry`（推荐）**

```rust
// 输入既有的 credential_source / usage_transport 两个 seam（依赖注入，不自建）
struct ClaudeAccessToken<'a> { /* 借用 credential_source + usage_transport + app_config */ }

impl ClaudeAccessToken<'_> {
    /// 步骤 1-3：读凭据 → scope-gate → 过期则刷新并持久化。
    async fn acquire(&self) -> Result<(ClaudeCodeCredentials, String), ClaudeAuthError>;

    /// 步骤 4：执行闭包；遇 AuthRequired 则刷新一次并重试。
    async fn with_auth_retry<T, F>(&self, creds: &ClaudeCodeCredentials, token: String, call: F)
        -> Result<T, ClaudeAuthError>
    where F: Fn(String) -> Fut;
}
```

- quota 的 `quota_snapshot` 用 `acquire` + `with_auth_retry(|t| usage(t))` 替换 726-782 的手写流程，再把 `Result` map 成 snapshot。
- trigger 的 `claude_access_token` 删除，`list_models`/`trigger` 改用 `acquire` + `with_auth_retry(|t| fetch_models(t))` / `(|t| trigger(t))`。

**错误类型**：定义一个 `ClaudeAuthError`（`NoCreds` / `MissingScope` / `RefreshRejected` / `Terminal(String)`），由各调用方 map 到自己的领域错误——quota → `ProviderQuotaSnapshot` 的 `status`，trigger → `ProviderTriggerError`。scope 文案只活在这一个枚举里。

## Suggested shape

- **接受依赖、不创建依赖**：module 借用现有的 `ProviderCredentialSource` 与 `ProviderUsageTransport` 两个 seam（它们已经是 quota 暴露给 trigger 的接口），从而天然可测——注入 fake transport 就能驱动「刷新成功 / 刷新被拒 / 401 后重试成功」三条路径。
- **放置位置**：建议落在 provider_quota module 内（如 `provider_quota/claude_auth.rs`），与 `[[260627-1838-provider-quota-按-provider-垂直切分]]` 的 `providers/claude_code.rs` 相邻；trigger 侧 `use` 它。这样 Claude 凭据知识有单一 locality。
- **不要把它做成泛 Provider 抽象**：目前只有 Claude 走 OAuth 刷新这套；codex/copilot 是静态 token。"two adapters means a real seam"——只有 Claude 一家，就保持它是 Claude 专属深 module，不强行泛化。

## Before / After

```text
BEFORE
  provider_quota::quota_snapshot   ┐  各自手写：
  provider_trigger::claude_access_token + 内联重试 ┘  读凭据→scope-gate→刷新→401重试
  scope 文案逐字重复 ×2

AFTER
  ClaudeAccessToken::acquire / with_auth_retry   ← 单一 locality
        ↑                    ↑
   quota 调用            trigger 调用     （文案/刷新时机/重试策略只存一份）
```

## Acceptance criteria

- [ ] scope 校验文案 `user:profile ... claude setup-token` 在代码库中只出现一次。
- [ ] quota 的 `ClaudeCodeQuotaAdapter::quota_snapshot` 不再手写 scope-gate / expiry-refresh / auth-retry，改为调用共享 module。
- [ ] trigger 的 `ClaudeCodeTriggerRunner` 不再有独立的 `claude_access_token` / `refresh_or_auth_error` 手写实现。
- [ ] 共享 module 通过注入 fake `ProviderUsageTransport` 覆盖：刷新成功、刷新被拒、401 后重试成功三条路径。
- [ ] `tests/provider_quota.rs` 与 provider_trigger 现有测试全绿；外部可见错误语义不变。

## Out of scope

- 不改 `ProviderCredentialSource` / `ProviderUsageTransport` 两个 seam 的签名。
- 不把 codex/copilot 等静态 token provider 塞进这套 OAuth 编排。
- 不改 keychain 读写或 OAuth refresh 的 HTTP 细节（仅收敛编排层）。

## Notes

与 `[[260627-1838-provider-quota-按-provider-垂直切分]]` 同属一次 provider_quota 内部深化，建议**先做垂直切分、再抽这个 Claude 编排 module**——切分后 Claude 的两处实现都更短、更易对照差异，抽取风险更低。
