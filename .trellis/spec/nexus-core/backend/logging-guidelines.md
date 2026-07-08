# nexus-core Logging Guidelines

## 当前模式

`nexus-core` 不使用全局 logging facade。外部请求相关可观测性通过 `services/outbound_request_log.rs` 的 `OutboundRequestLogger`，由 Tauri `AppState` 注入到 Provider / Sync / Session 等 service。

参考文件：

- `src-tauri/src/store.rs`：创建并传递 `OutboundRequestLogger`。
- `crates/nexus-core/src/services/outbound_request_log.rs`。
- `crates/nexus-core/src/services/provider_quota/`、`services/webdav.rs`、`services/sync/task_lifecycle.rs`。

## 记录边界

- 记录外部请求时应保留调试所需的 provider / URL 类别 / status / 错误摘要。
- WebDAV、Provider quota、Cloud Session 等网络边界优先通过 request logger，而不是 `println!`。
- 测试可使用 `OutboundRequestLogger::for_test()`；参考 `tests/sync_service.rs`。

## 敏感信息

不要记录：

- WebDAV password。
- Provider API key、cookie、Copilot token、OpenCode Go workspace/auth 材料。
- Prompt / Session 正文。
- 用户本地完整路径，除非该路径本身是功能输出且必要；优先记录摘要或已脱敏路径。

## 常见错误 / anti-pattern

- 在 provider adapter 中打印原始 headers。
- 为了排查测试失败向 stdout 输出大量临时日志并提交。
- 在 core 中引入 Tauri logging API；`nexus-core` 必须保持无 Tauri 依赖。
