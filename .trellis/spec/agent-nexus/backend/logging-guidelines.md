# Tauri Backend Logging Guidelines

## 当前日志方式

项目当前没有引入 tracing/log facade。Tauri 壳层主要使用：

- `eprintln!`：后台调度错误摘要，见 `src-tauri/src/lib.rs` 的 `start_background_scheduler`。
- `OutboundRequestLogger`：由 `src-tauri/src/lib.rs` 从 app data dir 创建，并传入 Provider / Sync / Session 相关 service。
- 前端 dev-only `console.debug`：桌面 host 连接状态见 `src-react/src/App.tsx`。

## 记录什么

- 后台 scheduler 失败应记录任务类型和错误摘要，保证下一分钟仍继续轮询。
- 外部 HTTP / WebDAV 请求日志通过 `nexus-core` 的 `OutboundRequestLogger`，不要在 Tauri 层重复打印请求细节。
- App setup 失败可以由 `?` 传播到 Tauri 启动错误。

## 不记录什么

- 不记录 WebDAV password、Provider token、cookie、Copilot token、OpenCode Go connection params 等敏感值。
- 不把完整用户资产正文（Prompt / Session body）写入日志。
- 不在普通成功路径刷屏输出；桌面应用默认应安静运行。

## 常见错误 / anti-pattern

- 在 command 中临时 `println!` 调试并提交。
- 在 `providers` / `sync` 路径中打印原始 request headers。
- 在后台线程 panic；应该捕获错误并继续下一轮。

## 参考文件

- `src-tauri/src/lib.rs`
- `src-tauri/src/store.rs`
- `crates/nexus-core/src/services/outbound_request_log.rs`
