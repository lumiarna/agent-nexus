# Provider 窗口对齐自动触发

## 背景

Claude Code / CodeX 等 Provider 的 5h 滚动窗口以**首次使用**为起点。用户工作时段（08:00 起）若在该时刻才发请求，窗口要等 5h 后（13:00）才 reset，期间额度可能不够用。

**目标**：让 Agent Nexus 在每天固定时间点（默认 05:00 / 10:00 / 15:00 / 20:00，可通过 cron 配置）自动向 Provider 发一个最小成本的占位请求。

## 现状

- BR §171-173 已写"Provider 额度定时重置"为产品目标，但**未实现**
- `ROADMAP.md:6` 已写"通过设置 cron 表达式自动触发对话以刷新 quota 窗口"
- `lib.rs:92` 后台调度线程已就位（驱动 Sync 任务），可挂新调度
- `cron.rs` CRON 解析与匹配已实现
- 缺：`ProviderTrigger` port、`provider_trigger_schedule` 表、UI 入口、触发函数

## 关键约束（必须诚实告知用户）

- **必须发真实推理请求**——`oauth/usage` 等查询端点不重置窗口起点；要重置必须发推理请求
- **模型动态拉取 + 用户手选**——硬编码 model id 会过期失效（实测：Claude OAuth 可见清单里 `haiku-3.5` 已下架，最低 haiku 是 `claude-haiku-4-5-20251001`）。改为各 provider 动态列出可用模型，用户在 UI 手选；失效时用户重选一次。详见「实测结论」
- **有最小计费**——约 100 tokens/请求保底；每天 4 次约 0.0012 USD，成本可忽略但**不是零**
- **失败要可见**——token 失效 / quota 已用完 / agent 未启动都会导致触发失败，必须 UI 提示
- **quota 已用完时触发失败**——这是预期行为，但用户必须知道

## 实测结论（2026-06-26 用本地真实凭据实拿，非推测）

实测 3 个 provider 的「模型清单 + 价格」数据源：

| Provider | 模型清单动态 | 价格/成本 | 端点 |
|----------|------------|----------|------|
| Claude Code (OAuth) | ✅ 9 个 | ❌ 无 `pricing` 字段 | `GET /v1/models` + `anthropic-beta: oauth-2025-04-20` |
| Copilot | ✅ 26 个 | ❌ `billing: null` | `GET /models`（先用 OAuth 换 copilot bearer）|
| OpenRouter | ✅ 339 个 | ✅ USD/token（`pricing.prompt/completion`，需滤 `-1` 动态价）| `GET /api/v1/models`（公开）|

**三条改变设计的结论**：

1. **硬编码 model id 会失效** — Claude OAuth 可见 9 模型中 `haiku-3.5` 已下架，实际最低 haiku 为 `claude-haiku-4-5-20251001`。→ 必须动态拉清单，不能写死。
2. **价格档无统一源** — 8 个 provider 仅 OpenRouter 有 USD 价格，Claude/Copilot 实测均无成本字段。→ 放弃价格/成本标注，改用户手选。
3. **模型清单动态拉取可行** — Claude/Copilot/OpenRouter 实测都能列；OpenCode Go 无端点 → `supported=false`。

Claude OAuth 实测可见模型 id：`claude-fable-5` / `claude-opus-4-8` / `claude-opus-4-7` / `claude-sonnet-4-6` / `claude-opus-4-6` / `claude-opus-4-5-20251101` / `claude-haiku-4-5-20251001` / `claude-sonnet-4-5-20250929` / `claude-opus-4-1-20250805`。

## 跨项目参考：agent-quota-monitor 的"意外副作用"

agent-quota-monitor 是**只读观测器**（CLAUDE.md 明确"不写任何凭据文件"），但**Claude Code 路径上已经"碰巧"实现了 5h 窗口对齐**——不是有意，是 fallback 路径的副作用。

**`claude.rs:359-462` `fetch_usage_with_fallback` 流程**：

1. 先打 `GET https://api.anthropic.com/api/oauth/usage`（纯观测，不重置窗口）
2. **如果 `resets_at` 字段缺失** → 走 fallback `fetch_usage_via_messages`
3. **`fetch_usage_via_messages` 实际是 `POST https://api.anthropic.com/v1/messages`**，带 OAuth bearer / `anthropic-version: 2023-06-01` / `anthropic-beta: oauth-2025-04-20` / `max_tokens = 1` / `messages: [{"role": "user", "content": "."}]`
4. 模型链 `["claude-3-haiku-20240307", "claude-haiku-4-5-20251001"]`，找到有 rate-limit 头就返回

**关键点**：这条 fallback 路径**每次轮询都会发一次真实推理请求**（如果 `resets_at` 缺失或 `oauth/usage` 端点失败），会消耗配额、推进 5h 窗口起点。这是 agent-quota-monitor 5h 窗口"看起来会自动对齐"的根因——**不是设计，是 fallback 副作用**。

**对我们的参考价值**：
- ✅ **endpoint 选型验证**：`POST /v1/messages` + `max_tokens=1` 实测可用，不会被 OAuth 端点拒
- ✅ **请求头参考**：`Authorization: Bearer <token>` + `anthropic-version` + `anthropic-beta: oauth-2025-04-20`
- ✅ **模型链**：`claude-haiku-4-5-20251001` 当前最低 haiku（issue 文档「实测结论」已确认）
- ⚠️ **不要照搬写死 model id**：agent-quota-monitor 写死 `claude-3-haiku-20240307` 是历史包袱，本 issue 仍要求**动态拉清单 + 用户手选**
- ⚠️ **agent-quota-monitor 的 fallback 路径是隐藏配额消耗源**——用户不知情。issue 实现必须**显式 opt-in + 失败可见**，避免重蹈覆辙

**对应文件**：
- `agent-quota-monitor/src/providers/claude.rs:424-462` `fetch_usage_via_messages`（fallback 路径）
- `agent-quota-monitor/src/providers/claude.rs:359-384` `fetch_usage_with_fallback`（调用入口）
- agent-nexus 的 `crates/nexus-core/src/services/provider_quota.rs` 当前**没有等价 fallback**，因此**没有这个副作用**——这是 agent-nexus 在 Claude Code 上"窗口不会自动对齐"的根因。

## 待实测：CodeX `wham/usage` 是否也有触发窗口的副作用

**怀疑**：`https://chatgpt.com/backend-api/wham/usage` 是 ChatGPT 后端私有端点（不在 OpenAI 公开 API 文档），可能**在响应 quota 快照的同时把这次"调用"也计入 5h 窗口**——即"观测即触发"。

**当前证据**：
- ✅ agent-quota-monitor 与 agent-nexus 都打 `https://chatgpt.com/backend-api/wham/usage`（端点完全相同）
- ✅ 两边都看不到自动触发效果（实测窗口不会因为轮询而"重置"）
- ❌ 但**没有官方文档佐证** `wham/usage` 不消耗配额——OpenAI 公开 rate-limits 文档只讲 RPM/TPM，不讲 5h 窗口
- ❌ 用户实际感觉"CodeX 5h 后自动归 0"**可能是自然到期**（窗口本身有 5h 寿命），**不是观测触发**

**实施本 issue 时需要实测确认**：
1. **不轮询只观测** vs **每分钟轮询**——quota reset 时间是否一致
2. **轮询 `wham/usage`** vs **调用 `v1/chat/completions`**——5h 窗口归 0 行为是否一致
3. 实测 24h 数据：每 5 分钟打一次 `wham/usage`，看 `rate_limit.primary_window.reset_at` 是否被推进

**结论暂定**：**先按"wham/usage 不触发窗口"实现**（与现状一致），如果实测发现会触发，**在 trigger 路径里复用 `wham/usage` 当作"零成本"触发器**，**完全不发推理请求**。但**不要在实测前**假设这一点。

## 实现重点

### 1. 后端 — `crates/nexus-core/src/services/provider_trigger.rs`（新文件）

- 定义 `ProviderTrigger` port trait（含 `list_models` + `trigger` 两个动作），复用 `ProviderUsageTransport` 同款 HTTP 客户端
- **PoC 先实 Claude Code**（`list_models` + `trigger` 都做），其余 provider 留 TODO + `supported=false`
- `list_models`：动态拉取该 provider 可用模型清单（实测可行 — Claude `/v1/models`、Copilot `/models`、OpenRouter `/api/v1/models`）
- 触发函数构造最小调用：`model = 用户选定模型（来自 list_models）` / `prompt = "hi"` / `max_tokens = 1` / `stream = false`
  - ⚠ 各 provider 实现时先验证 `max_tokens = 1` 不被端点拒绝（部分 API 有下限），这比抠 prompt 选词省 token 重要一个数量级
- Auth 路径复用 `LocalCredentialSource`（已在 `provider_quota.rs` 实现）
- 触发结果用 `Result<ProviderTriggerOutcome, AppError>` 返回：`Success { model, prompt_tokens, completion_tokens }` / `AuthExpired` / `QuotaExhausted` / `NetworkError`

### 2. 数据库 — Schema v11 migration

新增 `provider_trigger_schedule` 表：

- `provider_id TEXT PRIMARY KEY`（关联 provider）
- `enabled INTEGER NOT NULL DEFAULT 0`（opt-in 默认）
- `cron_expr TEXT NOT NULL DEFAULT '0 5,10,15,20 * * *'`
- `prompt_template TEXT NOT NULL DEFAULT 'hi'`
- `model_id TEXT`（用户从动态清单选定的模型 id；`enabled=1` 时必须非空，否则配置不完整、不触发并提示。**不再有"NULL 写死最便宜"语义** — 实测证明写死会过期失效）
- `last_trigger_at INTEGER`
- `last_status TEXT`（`success` / `auth_expired` / `quota_exhausted` / `network_error` / `never`）
- `last_error TEXT`

在 `schema.rs` 加 `migrate_to_v11`，按现有 `migrate_to_vN` 模式独立事务。

### 3. 调度 — `crates/nexus-core/src/services/sync.rs`（扩）或新 `trigger_scheduler.rs`

- 复用 `lib.rs:92` 的"每分钟对齐边界轮询"模式
- 加 `run_due_trigger_schedules(now)`：查 `enabled=1 AND cron 命中 now` 的 provider，逐个调 trigger
- 触发后更新 `last_trigger_at` / `last_status` / `last_error`

### 4. Tauri 命令 — `src-tauri/src/commands/providers.rs`

- `get_provider_trigger_schedule(provider_id)` → 读 settings
- `set_provider_trigger_schedule(provider_id, schedule)` → 写 settings
- `list_provider_trigger_models(provider_id)` → 动态返回该 provider 可选模型清单（供前端下拉）；`supported=false` 的 provider 返回空 + 不可用标识
- `trigger_provider_now(provider_id)` → 立即触发一次（不依赖 cron），供 UI "立即触发" 按钮
- `get_provider_trigger_status(provider_id)` → 返回上次触发结果

### 5. 前端 — `src-react/src/components/provider/ProviderPage.tsx`

- **全部 provider 卡片**都加 "Window alignment" 入口（独立图标按钮，**不复用现有 ⚙️ 配置弹层** — 那个副标题是 "observation params · not a credential manager"，与"主动发计费请求"语义冲突）
- `supported=false` 的 provider（如 OpenCode Go）入口置灰 + "Coming soon"，不静默失效
- 弹层：开关 + cron 输入（复用 Sync 页 `Segmented + 预设 + cronHuman` 模式，加 `05/10/15/20` 预设）+ prompt 模板（默认 `hi`）+ 模型下拉（选项来自 `list_provider_trigger_models`，**无 "Auto"，用户手选**）
- 模型未选时禁止打开开关（`enabled=1` 必须有 `model_id`）
- 底部显示"上次触发：3h 前 ✓ 消耗 8 tokens"或"上次触发：5/26 20:00 ✗ Quota 已用完"
- 立即触发按钮（用于测试）
- 卡片本体加失败指示（仅失败时一行小 badge）— 满足"失败不静默"，不打开弹层即可见

### 6. PoC 策略

**前端铺全 provider，后端先只实 Claude Code**（由 `supported` 标识控制其余 provider 入口可用性，避免"前端铺满 ≠ 功能铺满"的误导）。Claude Code 跑 3 天验证：

- 触发成功率（Agent 不在运行时不计）
- 实际 token 消耗
- 5h 窗口 reset 时间是否真的稳定锚定到 10:00 / 15:00 / 20:00 / 01:00

验证通过后后端逐个补 CodeX / Copilot / OpenCode 自定义 的 `list_models` + `trigger`。

## 已定决策（本轮敲定）

- **模型选择**：动态拉清单 + 用户手选，**无 Auto，不标价格**（实测仅 OpenRouter 有 USD、无统一源）
- **占位 prompt**：定 `"hi"`。`max_tokens=1` 已把 output 钉死 1 token，唯一变量是 input：`hi`≈1tok < `Reply with OK.`≈4tok，故 `hi` 更省（属噪音级优化，列为可改字段）
- **前端范围**：铺全 provider，后端 `supported` 标识控制可用性

## 决策待定

- **opt-in vs 默认开启**：建议 opt-in（与 BR 隐含语义一致，避免"用户不知情地每天消耗 tokens"）
- **cron 默认值**：5/10/15/20 点（与 BR §171 一致）还是用户可改（更灵活）
- **失败重试策略**：失败后下一次 cron 再试（简单）还是立即重试 1 次（更稳但要避免循环）
- **模型下拉默认选中**：无价格数据 → 默认不预选、强制用户选一次，还是默认选清单第一个？

## 验收标准

- [ ] Schema v11 migration 落地，已有库可平滑升级
- [ ] `ProviderTrigger` port（`list_models` + `trigger`）落地；Claude Code 两者都实现，其余 provider 留 TODO + `supported=false`
- [ ] `list_provider_trigger_models` 动态返回真实清单（Claude 实测 9 个），前端下拉消费
- [ ] 调度线程按 cron 命中调用触发，结果回写 `provider_trigger_schedule`
- [ ] 前端**全 provider** 卡片显示窗口对齐入口，弹层可配置 cron / prompt / model（下拉）
- [ ] UI 显示"上次触发时间 / 状态 / 消耗 tokens / 错误"
- [ ] 失败时触发 fallback 错误提示，不静默失败（卡片本体 + 弹层双处可见）
- [ ] `supported=false` 的 provider 入口置灰 "Coming soon"，不静默失效
- [ ] Rust 单元测试 + 集成测试覆盖：触发成功 / auth 过期 / quota 用完 / 网络错 / 5h 窗口未过期时是否仍触发并重置
- [ ] TS 组件测试：cron 编辑器、模型下拉、状态显示、立即触发按钮

## 不做的事

- **不接管 provider 身份生命周期**——trigger 失败时不重试登录，让用户去原 agent 客户端
- **不发"占位探针"**——`oauth/usage` 之类查询不重置窗口，必须发推理请求
- **不承诺"窗口对齐 100% 准确"**——用户自己使用行为不可控，文档明示"对齐效果取决于你后续的使用模式"
- **MVP 不做"按 quota 用量自动暂停触发"**——quota 用完时仍按 cron 尝试（失败可见即可），不做智能节流
- **不做价格/成本档抓取**——实测仅 OpenRouter 有 USD 价格（还要滤 `-1` 动态价），Claude/Copilot 无成本字段、无统一源；用"动态清单 + 用户手选"替代成本保护
- **不自动识别"最便宜模型"**——所有 provider（含内置）都由用户从动态清单手选，不写死 model id（会过期）；`enabled=1` 时 `model_id` 必须非空

## 参考

- BR §171-173 — "Provider 额度定时重置"产品目标
- `ROADMAP.md:6` — "每天 5/10/15/20 点自动触发对话"
- `crates/nexus-core/src/services/provider_quota.rs` — `LocalCredentialSource` 与 `HttpUsageTransport` 同款可复用
- `crates/nexus-core/src/services/cron.rs` — 纯 cron 解析与匹配
- `src-tauri/src/lib.rs:92` — 现有调度线程模式
- `prototype/Sync.dc.html:226` — cron 输入控件原型
- `docs/design/Provider Quota.md` — 各 provider auth 来源
