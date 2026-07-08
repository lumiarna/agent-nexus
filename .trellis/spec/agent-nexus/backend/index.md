# Agent Nexus Tauri Backend 规范

本目录描述 `src-tauri/` Tauri 壳层的开发约定。业务领域逻辑属于 `crates/nexus-core/`；Tauri 层只负责应用装配、命令暴露、状态注入、托盘/窗口和后台调度入口。

## 规范清单

| Guide | 用途 |
|-------|------|
| [Directory Structure](./directory-structure.md) | `src-tauri/src` 模块职责和 command/service 边界 |
| [Database Guidelines](./database-guidelines.md) | Tauri 层如何打开并共享 `nexus-core::Database` |
| [Error Handling](./error-handling.md) | `AppResult` 透传、命令边界错误处理 |
| [Quality Guidelines](./quality-guidelines.md) | Tauri 壳层验证、避免业务逻辑下沉到 command |
| [Logging Guidelines](./logging-guidelines.md) | 当前日志方式、后台任务和敏感信息规则 |

## 核心边界

- `src-tauri` 是 shell，不是领域层；参考 `docs/design/Architecture Design.md`。
- `AppState` 在 `src-tauri/src/store.rs` 装配 `nexus-core` services。
- Tauri commands 在 `src-tauri/src/commands/`，模式是“接收参数 → 调用 service → 返回 `AppResult<T>`”。
