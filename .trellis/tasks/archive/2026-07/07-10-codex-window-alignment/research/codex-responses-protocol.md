# CodeX Responses 协议研究

日期：2026-07-10

## 结论

CodeX CLI 当前的 ChatGPT 登录模式使用 Codex 专用 base URL：

- 默认 base URL：`https://chatgpt.com/backend-api/codex`
- 模型列表：`GET /models?client_version=<version>`，完整 URL 例如 `https://chatgpt.com/backend-api/codex/models?client_version=0.0.0`
- 推理请求：`POST /responses`，即 `https://chatgpt.com/backend-api/codex/responses`
- 模型列表 JSON 外层为 `{ "models": [...] }`，模型字段至少含 `slug`、`display_name`、`supported_in_api` 等；Agent Nexus 只需映射 `slug -> id` 和 `display_name`。
- CodeX CLI 使用 Responses API，发送 `stream: true`；触发器可以发送一个最小输入并消费完整响应流，不能只依赖 quota 查询。

## 凭据与请求头

仓库已有 CodeX `auth.json` 解析和 `tokens.access_token` / `tokens.account_id` 读取逻辑。CodeX CLI 的 ChatGPT auth 请求使用 bearer token，并在存在 account id 时加入 `ChatGPT-Account-Id`。Agent Nexus 应继续复用已有读取逻辑，不写入或刷新凭据。

## 参考证据

以下是 2026-07-10 从 OpenAI Codex 官方仓库 `main` 分支读取的源码：

- `codex-rs/model-provider-info/src/lib.rs`
  - `CHATGPT_CODEX_BASE_URL = "https://chatgpt.com/backend-api/codex"`
  - ChatGPT auth 默认使用该 base URL。
- `codex-rs/model-provider/src/models_endpoint.rs`
  - `MODELS_ENDPOINT = "/models"`
  - 使用 `client_version` query 参数。
- `codex-rs/codex-api/src/endpoint/models.rs`
  - `ModelsResponse { models: Vec<ModelInfo> }`
  - `ModelInfo` 通过 `slug` 和 `display_name` 提供模型选择信息。
- `codex-rs/codex-api/src/endpoint/responses.rs`
  - Responses 请求路径为 `responses`，HTTP 方法为 POST。
- `codex-rs/core/src/client.rs`
  - ChatGPT 登录的 Responses 请求默认 `stream: true`、`store: false`（非 Azure）、输入来自 Responses `input`。

源码地址：

- https://github.com/openai/codex/blob/main/codex-rs/model-provider-info/src/lib.rs
- https://github.com/openai/codex/blob/main/codex-rs/model-provider/src/models_endpoint.rs
- https://github.com/openai/codex/blob/main/codex-rs/codex-api/src/endpoint/models.rs
- https://github.com/openai/codex/blob/main/codex-rs/codex-api/src/endpoint/responses.rs
- https://github.com/openai/codex/blob/main/codex-rs/core/src/client.rs

## 实现边界

- 端点和协议细节只放在 `nexus-core` 的 CodeX trigger runner / adapter 中；调度 service、Tauri command 和前端继续使用 provider-neutral 的现有契约。
- 模型列表应过滤 `supported_in_api == false`，并按稳定顺序（优先级后名称/id）返回。
- 触发器使用最小用户输入（现有全局约定为 `.`），`max_tokens` 不属于 Responses API；应以 `stream: true`、空 tools、`store: false` 的最小 Responses 请求为基础，并消费到流结束。
- HTTP 2xx 但 SSE 内有 `response.failed` / `error` 时，必须转为失败；401/403 应视为认证失败，429/5xx/网络错误按现有 retryable/terminal 语义映射。
- 模型列表的 `client_version` 使用 Agent Nexus 自身固定兼容版本字符串，不读取或依赖 CodeX 私有安装目录中的版本。
