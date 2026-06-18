# Clippy lint 欠债

## 问题

`pnpm rust:test` 对应的 `cargo clippy --all-targets` 报 6 个 pre-existing warning，非本次修复引入，但未被清理。

## 位置

- `src-tauri/src/database/schema.rs` 5 处 `Result.or_else(|x| Err(y))`，应改为 `map_err(|x| y)`
  - 第 59 行（migrate_to_v1）
  - 第 247 行（migrate_to_v2）
  - 第 275 行（migrate_to_v3）
  - 第 312 行（migrate_to_v4）
  - 第 343 行（migrate_to_v5）
- `src-tauri/src/services/symlink.rs` 第 50 行 `redundant closure`

## 修复建议

一次性 cleanup：`cargo clippy --fix --lib -p agent-nexus` 可自动修复大部分，剩余手动调整。修完后跑 `cargo clippy --all-targets -- -D warnings` 确认零 warning。

## 备注

schema.rs 的 `or_else` 模式从 v1 沿用到 v5，v5 沿用是为了与现有迁移函数风格保持一致。修复时应统一改为 `map_err`。
