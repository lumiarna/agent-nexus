# Tauri Backend Quality Guidelines

## 核心质量门槛

- Tauri 层必须保持薄壳：命令注册、状态注入、窗口/托盘、后台调度可以在 `src-tauri`，领域逻辑进入 `nexus-core`。
- 新 command 需要同时更新 `commands/mod.rs`（如需要）、具体 `commands/<domain>.rs` 与 `lib.rs` 的 `generate_handler!`。
- 新 service 需要在 `store.rs` 的 `AppState` 中统一装配，避免重复实例。

## 验证建议

- Rust 格式化遵守 `GOTCHAS.md`：单文件修复可先 `cargo fmt --all -- <path>`，最终验证再跑 `pnpm rust:fmt`。
- 涉及 `nexus-core` 测试时，Windows 下使用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`。
- 如果只改 Tauri command glue，至少运行 Rust fmt/check；如果改到 core service，运行对应 core tests。

## Code review checklist

- Command 是否只做参数接收和 service 调用？
- 是否返回 `AppResult<T>` 而不是手写字符串错误？
- 是否没有新增 Tauri 依赖到 `crates/nexus-core`？
- 是否没有在 Tauri 层复制 `CONTEXT.md` 中的领域不变量？
- 是否避免记录 secret 或用户资产正文？

## 常见错误 / anti-pattern

- 把 Sync `Direction`、`Action`、`1 source → 1 target` 等规则写在 command 中。
- 为了前端方便暴露任意 target path；`CONTEXT.md` 要求 target path 由系统按 agent 与上下文计算。
- 删除或隐藏 UI 对应 command，只因为后端暂未完整实现；`GOTCHAS.md` 允许 UI 超前。
