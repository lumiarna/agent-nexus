# Tauri Backend Directory Structure

## 适用范围

适用于 `src-tauri/src/`。这里是 Tauri 2 壳层，不承载 Agent Nexus 领域规则。

## 本项目结构

```text
src-tauri/src/
├── main.rs              # 入口，只调用 agent_nexus_lib::run()
├── lib.rs               # Tauri Builder、setup、命令注册、窗口事件、后台调度线程
├── store.rs             # AppState：装配 nexus-core services
├── tray.rs              # Windows/taskbar tray 管理
└── commands/
    ├── mod.rs
    ├── app.rs / app_config.rs / agent_capabilities.rs
    ├── projects.rs / project_symlinks.rs
    ├── skills.rs / prompts.rs / sessions.rs
    ├── providers.rs / sync.rs / tray.rs
```

参考文件：`src-tauri/src/lib.rs`、`src-tauri/src/store.rs`、`src-tauri/src/commands/projects.rs`、`src-tauri/src/commands/sync.rs`。

## Command / Service 边界

- 新 Tauri command 放入对应 `commands/<domain>.rs`，并在 `lib.rs` 的 `tauri::generate_handler!` 注册。
- Command 保持薄：参数使用 Rust 类型接收，直接委托 `state.<service>`，返回 `AppResult<T>`。
- 领域验证、事务、文件系统 placement、WebDAV、Provider quota 等逻辑应在 `crates/nexus-core/src/services/`。

## AppState 装配

- `store.rs` 用 `Arc<Database>` 共享同一 `nexus-core::Database`。
- 新增 service 时，在 `AppState` 字段和 `AppState::new` 中装配；不要在 command 中临时打开数据库或构造重复 service。

## 常见错误 / anti-pattern

- 不要把 SQL、扫描规则、Sync 不变量写在 Tauri command 中。
- 不要绕过 `AppState` 自己打开 SQLite 文件。
- 不要把 `src-tauri` 与 `nexus-core` 的职责混写；`nexus-core` 必须保持无 Tauri 依赖。
