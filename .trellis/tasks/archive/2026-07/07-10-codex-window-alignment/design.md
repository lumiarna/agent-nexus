# CodeX Window Alignment 技术设计

## 1. 边界与总体方案

沿用现有 Window Alignment 深模块，不新增调度器、数据库字段或 Tauri API：

- `ProviderTriggerService` 保持 provider-neutral，继续负责能力查询、schedule 校验、5 小时冷却、并发 guard 和结果持久化。
- `ProviderTriggerRunner` 扩展为一个 runner 内的 CodeX adapter/分支；Claude runner 的行为保持不变。
- CodeX adapter 只负责 `auth.json` 凭据读取、`/models` 解析、`/responses` 请求构造和 HTTP/SSE 错误映射。
- Tauri command 继续透传现有 `ProviderTriggerCapability` / `ProviderScheduleSettings`，不增加 command。
- React 只把当前“仅 Claude”条件扩展为 Claude + CodeX；其他 provider 继续展示 unsupported 状态。

## 2. CodeX 外部协议

### 模型列表

使用 CodeX ChatGPT backend：

`GET https://chatgpt.com/backend-api/codex/models?client_version=<agent-nexus-version>`

请求头：Bearer access token、可选 `ChatGPT-Account-Id`、`Accept: application/json`。

响应 `{ models: [...] }`。仅返回 `supported_in_api == true` 的项，映射为既有 `ProviderTriggerModel { id: slug, displayName: display_name }`，按 `priority`、id 稳定排序。若返回项缺少可用 slug/display name，跳过该项；空列表仍是成功的 supported capability。

### 最小触发

使用：

`POST https://chatgpt.com/backend-api/codex/responses`

请求头：Bearer access token、可选 `ChatGPT-Account-Id`、`Content-Type: application/json`、`Accept: text/event-stream`。

请求体使用 Responses API 的最小兼容形态：

- `model`: 用户传入并经过 trim 的模型 id
- `instructions`: 空字符串
- `input`: 一个 user message，文本为现有全局最小 prompt `.`
- `tools`: 空数组、`tool_choice: "auto"`、`parallel_tool_calls: false`
- `reasoning`: null、`include`: []、`store`: false
- `stream`: true

响应必须被完整消费到流结束。HTTP 2xx 仅表示连接成功；若 SSE 中出现 `response.failed`、`response.error` 或含错误的 `error` event，则返回失败。成功可返回 token 统计（若 SSE usage 可解析），否则使用 0，不影响现有调度语义。

## 3. 认证与错误

- 通过已有 CodeX auth 解析路径读取 `tokens.access_token` 和 `tokens.account_id`；不写 auth 文件、不实现刷新、不启动 CodeX CLI。
- CodeX auth 读取应抽为 crate 内可复用入口，避免复制 `auth.json` JSON 结构解析；现有 quota adapter 和 trigger adapter共享它。
- 401/403 映射为 `AuthRequired`，沿用现有“terminal failure、用户去 CodeX login”的状态行为。
- 429/5xx/网络与响应流读取错误映射为 `Retryable`；其他非 2xx 或 malformed response 映射为 `Terminal`，复用已有 status/detail helper。
- outbound request 必须通过现有 `OutboundRequestLogger`，URL、headers 和错误详情遵循当前脱敏规则。

## 4. 跨层数据流

1. ProviderPage 打开 CodeX 配置时，`useProviderTriggerModelsQuery("codex", true)` 调 Tauri `list_provider_trigger_models`。
2. service 识别 runner 支持 CodeX，adapter 读取 auth 并请求 models，返回 capability。
3. 用户选择 start time/model；保存时 ProviderPage 对 `claude` 和 `codex` 都写入 `windowAlignCron` / `windowAlignModelId`。
4. 后端 schedule service 验证 active 配置，使用现有 runtime fields 安排下一次运行。
5. 后台调度或手动 command 调 runner；结果写入既有 schedule 状态，React Query mutation 更新当前 provider cache。

## 5. 兼容性与风险

- 不迁移数据库；既有 provider schedule 行可直接承载 `codex`。
- 旧版没有 CodeX window alignment 字段的行读取为默认 inactive，保存后正常工作。
- CodeX backend 是非公开协议，所有协议细节集中在 adapter 内，后续端点变化只需替换 adapter，不污染 service/UI。
- 不对 CodeX 模型做“最便宜”或硬编码选择；必须由用户选择动态列表中的模型。
- 若 CodeX models endpoint 暂时不可用，能力查询显示错误而非把 CodeX 标成 unsupported；只有 runner `supports("codex")` 决定能力标志。

## 6. 测试设计

- 纯解析测试：models 响应过滤/排序、Responses SSE 成功和失败事件、HTTP 状态映射、请求 JSON 与 account header。
- service 测试：fake runner 对 `codex` 的支持由现有调度测试覆盖，补充 active schedule 能保存并触发 codex provider id。
- 前端纯规则/组件测试：CodeX 可用时保存 window fields，Claude 不回归，unsupported provider 仍不发送 window fields；至少通过 typecheck 和现有 unit/component suite。
- 外部网络和真实 auth 均不进入测试；HTTP 行为使用 request logger 下可替换 transport 或纯解析 helper 验证。
