# Tauri Backend Database Guidelines

## 适用范围

适用于 `src-tauri` 如何使用数据库。具体 schema、迁移和 SQL 规范见 `nexus-core/backend/database-guidelines.md`。

## 本项目模式

- `src-tauri/src/lib.rs` 在 Tauri `setup` 中解析 `app.path().app_data_dir()`，打开 `agent-nexus.sqlite3`。
- `Database::open(...)` 来自 `nexus-core`，会创建父目录、打开 rusqlite connection 并执行迁移。
- `src-tauri/src/store.rs` 把 `Database` 包进 `Arc`，传给各领域 service。

参考文件：`src-tauri/src/lib.rs`、`src-tauri/src/store.rs`、`crates/nexus-core/src/database/mod.rs`。

## 规则

- Tauri 层只负责选择数据库文件位置；schema 和事务逻辑不得写在 `src-tauri`。
- Command 中不要拿 connection 执行 SQL；应调用 `state.projects`、`state.sync` 等 service。
- 新增需要持久化的功能时，优先在 `nexus-core` 增加 service / database 逻辑，再由 Tauri command 暴露。

## Windows SQLite 注意事项

- `docs/adr/0001-动态链接 SQLite.md` 决定 Windows 下动态链接官方 SQLite。
- `GOTCHAS.md` 明确不要直接裸跑 `cargo test -p nexus-core`；使用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`。
- 如果 `with-sqlite` 因运行中的旧 `agent-nexus` 占用 `sqlite3.dll` 而失败，可按 `GOTCHAS.md` 手动设置 `SQLITE3_LIB_DIR` 和 PATH 后直接运行 cargo，避免触碰用户进程。

## 常见错误 / anti-pattern

- 在 Tauri command 中硬编码数据库路径。
- 在 `src-tauri` 新增 migration。
- 在 Windows 文档或脚本建议裸 `cargo test -p nexus-core`。
