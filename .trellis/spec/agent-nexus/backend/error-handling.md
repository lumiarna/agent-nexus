# Tauri Backend Error Handling

## 适用范围

适用于 `src-tauri/src/commands/`、`src-tauri/src/lib.rs` 中 Tauri 边界的错误处理。

## Command 返回类型

- Commands 返回 `nexus_core::error::AppResult<T>`，让 Tauri 序列化 `AppError` 给前端。
- 不在 command 中把错误转换成字符串；保留 `AppError` 的结构化 kind。
- 参考 `commands/sync.rs`：`run_task` 返回 `AppResult<Task>`，直接 `state.sync.run_task(id).await`。

## App setup 错误

- `lib.rs` 的 `setup` 使用 `?` 传播 `app_data_dir`、`Database::open`、`OutboundRequestLogger` 初始化错误。
- `run(...).expect("failed to run Agent Nexus")` 只用于应用启动最外层；领域操作不要 `expect`。

## 后台任务错误

- `start_background_scheduler` 每分钟运行 Sync 和 Provider trigger。当前模式是捕获错误并 `eprintln!`，不能让后台线程 panic 退出。
- 后台任务不应记录 credential / cookie / token；只记录足够定位的错误摘要。

## 常见错误 / anti-pattern

- 在 command 中 `unwrap()` / `expect()` 处理用户路径、配置或 IO。
- 把 validation 写成前端-only；后端 service 必须返回 `AppError::Validation`。
- 为了“友好文案”丢弃 `AppError` kind，导致前端无法区分 validation / io / database。

## 参考

- `crates/nexus-core/src/error.rs`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/commands/sync.rs`
