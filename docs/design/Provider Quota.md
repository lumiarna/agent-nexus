# Provider Quota 获取方式

> 本文说明 Agent Nexus 中每个 `Provider` 的 quota 额度的获取逻辑与 auth 来源优先级。代码真相源：`crates/nexus-core/src/services/provider_quota.rs` 与 `app_config.rs`。

## 架构总览

Provider Quota 子系统采用 **adapter + ports** 分层：

- **外层 adapter** 按 `provider_id` 派发到具体 provider（Claude Code / CodeX / Copilot / OpenCode Go / Configured Provider / OpenCode 自定义）。
- **内层两个 port** 承担副作用：
  - `CredentialSource`：读取 auth（文件、Keychain、env、settings）。
  - `UsageTransport`：执行 quota HTTP 请求。
- **派生层**是纯函数：把 (auth, usage) → `ProviderQuotaSnapshot`。四种状态（`available` / `expired` / `failed` / `nocreds`）全部可经 fake adapter 在内存中测出，quota 接口自身不再触网。

派发顺序：

1. 4 个固定 adapter（Claude Code / CodeX / Copilot / OpenCode Go）
2. Configured Provider adapter（MiniMax Token Plan CN / DeepSeek / OpenRouter）
3. OpenCode 自定义 Provider：从 `opencode.json` 动态发现

固定 adapter 优先；只有固定 adapter 未匹配、且 ID 能从 `opencode.json` 找到时，才走自定义路径。

## Auth 来源优先级

下表汇总各 provider 的 auth 顺序。前序来源缺失或解析失败时，自动降级到下一级；全部缺失时状态为 `nocreds`。

| Provider | 1 | 2 | 备注 |
|----------|---|---|------|
| Claude Code | `<claude_config_dir>/.credentials.json` | macOS Keychain（仅 macOS） | OAuth，含 refresh token 自动续期 |
| CodeX | `<codex_config_dir>/auth.json` | — | OAuth |
| Copilot | `settings.COPILOT_GITHUB_TOKEN`（用户手动） | `${OPENCODE_AUTH_FILE}` 下 `github-copilot.{access,key}` | **不读** `${GITHUB_TOKEN}` / `${GH_TOKEN}` |
| OpenCode Go | `settings.OPENCODE_GO_WORKSPACE_ID` + `settings.OPENCODE_GO_AUTH_COOKIE` | — | 完全手动；workspace id 缺 `wrk_` 前缀时自动补齐 |
| Qoder | `settings.QODER_SESSION_COOKIE` | — | 个人账号手动粘贴 session cookie；不读 `QODER_PERSONAL_ACCESS_TOKEN`（CLI 用） |
| MiniMax Token Plan CN | `settings.PROVIDER_API_KEY_MINIMAX_TOKEN` | `${OPENCODE_AUTH_FILE}` 下 `minimax-cn-coding-plan.key/.access` | — |
| DeepSeek | `settings.PROVIDER_API_KEY_DEEPSEEK` | `${OPENCODE_AUTH_FILE}` 下 `deepseek.key/.access` | — |
| OpenRouter | `settings.PROVIDER_API_KEY_OPENROUTER` | `${OPENCODE_AUTH_FILE>` 下 `openrouter.key/.access` | — |
| OpenCode 自定义 Provider | `opencode.json` 中 `provider.<id>.options.apiKey` | — | 同文件读取 baseURL、npm、modelId |

路径默认值：

- `<claude_config_dir>` 默认 `~/.claude`（可由 `settings.CLAUDE_CONFIG_DIR` 改写）
- `<codex_config_dir>` 默认 `~/.codex`（可由 `settings.CODEX_CONFIG_DIR` 改写）
- `${OPENCODE_AUTH_FILE}` 默认 `~/.local/share/opencode/auth.json`
- `opencode.json` 路径：env `OPENCODE_CONFIG_FILE` > env `OPENCODE_CONFIG_DIR` 下 `opencode.json` > `~/.config/opencode/opencode.json`

`settings` 表来自 SQLite 内的 `settings` KV 表，用户凭据通过前端 Settings 页写入。

## 各 Provider 详情

### Claude Code

**auth 获取**：macOS 优先读 Keychain 的 `Claude Code-credentials` 条目；其他平台读 `<claude_config_dir>/.credentials.json`。

**OAuth 解析**：从 `claudeAiOauth`（兼容 `claude.ai_oauth`）子对象取 access/refresh/expiresAt/scopes。Plan 标签由 `subscriptionType` 或 `rateLimitTier` 推断（Max / Pro / Team / Enterprise）。

**续期策略**：token 在 60 s 内过期或 usage 端返回 401/403 时，自动用 refresh token 调 OAuth 端点续期，结果回写到原文件或 Keychain。缺少 `user:profile` scope 时直接 `failed` + 提示运行 `claude setup-token`。

**Quota 端点**：Anthropic OAuth usage 端点，Bearer + `anthropic-beta: oauth-2025-04-20`。

**窗口**：5h rolling + 7d weekly。`primary` 取最短窗口中最高的 used%。

### CodeX

**auth 获取**：读 `<codex_config_dir>/auth.json`，取 `tokens.access_token` / `tokens.account_id` / `tokens.id_token`。

**Plan 来源**：从 ID token（JWT）payload 的 `https://api.openai.com/auth.chatgpt_plan_type` 解出（Pro / Plus / Team / Enterprise / Business）。

**Quota 端点**：ChatGPT backend-api usage 端点，Bearer + `User-Agent: codex-cli`，有 `account_id` 时附带 `ChatGPT-Account-Id`。

**窗口**：`rate_limit.primary_window` + `secondary_window`。`limit_window_seconds` 决定标签：18000 → "5-hour limit"，604800 → "Weekly limit"。

### Copilot

**auth 顺序**：

1. 用户在 Settings 填的 `COPILOT_GITHUB_TOKEN`（非空优先）。
2. `${OPENCODE_AUTH_FILE}` 下 `github-copilot.access`（OAuth 形式）或 `github-copilot.key`（API key 形式）。

**Quota 端点**：GitHub `copilot_internal/user`，`Authorization: token <token>` + Editor-Version / Editor-Plugin-Version / X-Github-Api-Version 头。

**窗口**：`premium_interactions`（Premium Interactions）+ `chat`（Chat Quota），`reset_at` 由 `quota_reset_date`（calendar date，锚定 UTC 零点）派生。

**注意**：业务文档 `Business Requirement.md` 早期版本列出的 `${GITHUB_TOKEN}` / `${GH_TOKEN}` env 路径在实现里**不读**，已于本设计文档落稿时同步删除；credential hint 已对齐为 `settings.COPILOT_GITHUB_TOKEN`。

### OpenCode Go

**auth 来源**：完全由用户在 Settings 手动填 `OPENCODE_GO_WORKSPACE_ID` + `OPENCODE_GO_AUTH_COOKIE`，任一为空即 `nocreds`。

**Quota 端点**：`opencode.ai/workspace/<id>/go`，`Cookie: auth=<cookie>` + Chrome UA。

**Auth 失效判定**：401/403 视为 `expired`；此外若最终 URL 重定向到 `auth.opencode.ai` / `/authorize` / `/login`，也判定为 auth 失效。

**解析**：HTML 中抓 `rollingUsage` / `weeklyUsage` / `monthlyUsage` 三组对象，每组取 `usagePercent` + `resetInSec`。Plan 来自 HTML 中 `subscriptionPlan`，缺省 `"Go"`。

### Qoder

**auth 来源**：用户在 Settings 手动填 `QODER_SESSION_COOKIE`（DevTools 从 `qoder.com` 的 cookie 面板复制 `qoder_session_cookie` 的值）。空即 `nocreds`。**不**读 CLI 的 `QODER_PERSONAL_ACCESS_TOKEN`（那是 qodercli 二进制专用，与本端点无关）。

**Quota 端点**：`GET https://qoder.com/api/v2/me/usages/big_model_credits`。硬编码必要头：

- `Cookie: qoder_session_cookie=<user value>`
- `bx-v: 2.5.35`（阿里 baxia 风控 SDK 版本号；硬编码有版本漂移风险，详见踩坑记录）
- `x-csrf-token: _echo_csrf_using_sec_fetch_site_`（Next.js 风格的 CSRF echo 约定）
- `x-requested-with: XMLHttpRequest`
- `Referer: https://qoder.com/account/usage`
- `Sec-Fetch-Site: same-origin` / `Sec-Fetch-Mode: cors` / `Sec-Fetch-Dest: empty`
- Chrome 149 UA

**Auth 失效判定**：401/403 → `expired`，错误信息引导用户「从 qoder.com DevTools 重新粘贴 session cookie」。不在 browser 里跑、无法预知是否能稳定通过 Next.js CSRF 检查；写明 `bx-v` 是脆弱项。

**解析**：JSON 响应字段为：

```jsonc
{
  "user_id": "…",
  "quota_key": "big_model_credits",
  "status": "active" | "restricted",
  "plan_quota":           { "quota_summary": { "used_value", "limit_value", "usage_percentage", "unit" } },
  "resource_package_quota":{ "quota_summary": { "used_value", "limit_value", "usage_percentage", "unit" } },
  "total_quota":          { "quota_summary": { … } },
  "lastResetAt":   <unix-ms>,
  "nextResetAt":   <unix-ms>
}
```

DTO 用 `serde(rename_all = "snake_case")` 解析，并通过 `serde(alias = "nextResetAt")` / `serde(alias = "lastResetAt")` 兼容 camelCase 字段名。只取 `plan_quota.quota_summary` 与 `resource_package_quota.quota_summary`（+ `nextResetAt` → ISO8601）。**故意不取** `quota_detail[]` —— schema 演进风险（`source` 枚举日后可能扩），CLAUDE.md #8 拒绝启发式兜底。

**窗口**：
- 主窗 `Monthly limit`：`kind = Monthly`，`reset_at = unix_millis_to_iso(nextResetAt)`；`used` 优先取 `usage_percentage`，若缺失则回退到 `used_value / limit_value * 100`（`limit_value <= 0` 时整窗被丢弃，避免出现空百分比）。
- 资源包窗 `Resource pack`：仅当 `limit_value > 0` 时追加（避免显示「0 / 0」噪音），其余 schema 同上。
- 两个 window 都填 `value_label = "<used> / <limit> <unit>"`：`unit` 缺省时回落 `"credits"`；整数按千分位分隔（如 `17 / 3,000 credits`），小数保留两位自动去尾零。
- `primary` 始终为 `None` —— Qoder 卡片不展示顶部百分比主指标，只显示窗口中的明确用量数字。
- `kind = Monthly` + `reset_at` 让前端 `quotaDisplay` 推导出月度 pace marker（黑色竖线）。
- `status == "restricted"` **不**映射为 `Failed`，仍保持 `Available` —— Qoder 网页配额耗尽时仍正常渲染数字，行为一致。
- `plan` 字段固定为 `"Qoder"`（API 不返回）。

### MiniMax Token Plan CN

**auth 顺序**：用户填的 `PROVIDER_API_KEY_MINIMAX_TOKEN` 优先；否则从 `${OPENCODE_AUTH_FILE}` 读 `minimax-cn-coding-plan.key/.access`。

**Quota 端点**：MiniMax API，Bearer。

**业务校验**：`base_resp.status_code != 0` 视为 `AuthRequired`（不是 HTTP 状态码，是业务 status）。

**窗口**：从 `model_remains[].model_name == "general"` 取 5h（`current_interval_remaining_percent` + `end_time`）+ 7d（`current_weekly_remaining_percent` + `weekly_end_time`，仅在 `current_weekly_status == 1` 时展示）。

### DeepSeek

**auth 顺序**：用户填的 `PROVIDER_API_KEY_DEEPSEEK` 优先；否则从 `${OPENCODE_AUTH_FILE}` 读 `deepseek.key/.access`。

**Quota 端点**：必须走 **CloudFront 域名 + 手动 `Host: api.deepseek.com` 头**。原因：部分企业 DNS 把 `api.deepseek.com` 解析到被阻断的腾讯 EdgeOne CDN IP；走 CloudFront 域名 + 覆盖 Host 头可绕过 DNS 污染。详见 ADR-0002。

**窗口**：`balance_infos` → 每条 currency 一条 window，标签 `"<currency> balance"`，`value_only=true`（展示余额而非百分比）。`is_available=false` 时附加 `Insufficient balance` 错误。

### OpenRouter

**auth 顺序**：用户填的 `PROVIDER_API_KEY_OPENROUTER` 优先；否则从 `${OPENCODE_AUTH_FILE}` 读 `openrouter.key/.access`。

**Quota 端点**：OpenRouter credits 端点，Bearer。

**窗口**：`total_credits - total_usage` 拆成 `Credit used` + `Credit balance` 两条 value-only window。无余额时附加 `No credits remaining` 错误。

### OpenCode 自定义 Provider（动态）

仅当请求的 `provider_id` 不在固定 + configured 列表内、但能在 `opencode.json` 找到时进入此路径。

**Provider 发现**：`opencode.json` 路径按 env `OPENCODE_CONFIG_FILE` > `OPENCODE_CONFIG_DIR` 下 `opencode.json` > `~/.config/opencode/opencode.json` 顺序解析。`provider.<id>.options.baseURL`、`npm`、`options.apiKey` 任一为空即丢弃该 provider；`models` 中取第一个非空 model id；`name` 缺省回落到 `id`。

**auth**：仅用 `options.apiKey`（空即 `nocreds`），不从 settings 读取。

**Quota 端点**：根据 `npm` 字段派发：

- `@ai-sdk/openai-compatible` → `POST <baseURL>/chat/completions`（最小 chat 调用）
- `@ai-sdk/openai` → `POST <baseURL>/responses`（最小 responses 调用）
- 其他 → 报 `unsupported OpenCode provider package <npm>`

**解析**：抓响应头中所有 `x-token-count-*` 头，归一为 minute / hour / day / month 四档。`primary` 取 minute 窗口。月窗口的 reset 时间取下月 1 日 UTC 零点。

## 公共约定

### 状态映射

| 场景 | snapshot.status | 说明 |
|------|----------------|------|
| 401 / 403 | `expired` | auth 失效，UI 提示重登或换源 |
| 其他非 2xx / 网络错 / 解析错 | `failed` | 端点或网络问题，保留 error 详情 |
| 鉴权来源全空 | `nocreds` | credential 字段展示来源提示 |
| 200 + 解析成功 | `available` | 至少 1 个 window |

`snapshot.credential` 始终是**来源描述字符串**（如 `"~/.claude/.credentials.json"`、`"macOS Keychain · Claude Code-credentials"`、`"settings.COPILOT_GITHUB_TOKEN"`、`"opencode.json · <id>"`），不是凭据本体。

### 401 重试

- Claude Code：usage 端返回 401/403 时自动用 refresh token 续期并重试 1 次。
- 其他 provider：401/403 直接判定 `expired`，不重试。

### settings key 总览

用户可配置项（写入 `settings` 表）：

- `CLAUDE_CONFIG_DIR` / `CODEX_CONFIG_DIR` — 改写默认配置目录
- `COPILOT_GITHUB_TOKEN` — Copilot 手动 token
- `OPENCODE_GO_WORKSPACE_ID` / `OPENCODE_GO_AUTH_COOKIE` — OpenCode Go 凭据
- `QODER_SESSION_COOKIE` — Qoder 个人账号手动 session cookie
- `PROVIDER_API_KEY_MINIMAX_TOKEN` / `PROVIDER_API_KEY_DEEPSEEK` / `PROVIDER_API_KEY_OPENROUTER` — 三个 configured provider 的手动 API key

## 与 BR 文档的差异

下表仅列**尚未对齐**的差异；已对齐项（Copilot auth 来源、Copilot credential hint）已同步修正。

| 项 | BR 描述 | 实际实现 | 处理建议 |
|----|---------|----------|----------|
| MiniMax / DeepSeek / OpenRouter | BR 未列入 provider 清单 | 已实现 | 在 BR Provider 章节补列三条；细节留在本文 |
| OpenCode 自定义 Provider | BR 第 58 行提到"自动扫描出的其他 Provider" | 已实现，但只支持 npm = `@ai-sdk/openai-compatible` / `@ai-sdk/openai` | 在 BR 中明示支持范围 |
| OpenCode Go plan 标签 | 未指定 | 固定 `"Go"`，HTML `subscriptionPlan` 优先生效 | 在 BR 中标"默认 Go，以官方页面为准" |
| Qoder 个人账号 | BR 未列入 | 已实现（adapter + 个人 cookie + JSON endpoint） | 在 BR Provider 章节补列；风控脆弱性在本文踩坑记录中提示 |

## 不做的事

- **不接管第三方身份生命周期**：所有凭据只读不写（Claude OAuth refresh 是唯一例外，且仅在原文件 / Keychain 路径回写）。
- **不读 `${GITHUB_TOKEN}` / `${GH_TOKEN}` env**：历史上 BR 列过这两个路径但从未实现，已删除；保留此条作为"以后不要再加回去"的明确信号。
- **不做 quota 写入 WebDAV**：quota 是观测，不进 `Sync` 工作台。
- **不做 project-level quota 归因**：`Provider` 是 `Global Resource`（见 `CONTEXT.md`）。
