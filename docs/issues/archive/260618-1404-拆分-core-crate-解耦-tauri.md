# 拆分 core crate 以解耦 Tauri 与测试编译

## 问题

当前 `agent-nexus` 是单一 crate（`src-tauri/Cargo.toml`），`[lib]` 同时编译 `database` + `services` + `error` + `commands` + `store` + `lib.rs`。问题：

1. **测试编译拖整棵 Tauri**：`#[cfg(test)]` 的集成测试（`tests/sync_service.rs` 等 `use agent_nexus_lib::{database, services}`）会把 `tauri 2.8` 及其全部依赖（wry、webview2、windows、ring…）链进测试二进制。冷重编译 >10 min，增量改动也需重链 cdylib。
2. **死锁陷阱的系统性根因**：`Database` 是单连接 + `Mutex`，`SyncService` 等服务在同一方法内多次 `self.db.connection()` 会死锁（本次 `add_task` 已踩）。这是架构层面的隐患，不是单点修复能消除的——只要服务持有 `Arc<Database>` 且方法内并发查询，就随时可能复发。
3. **测试无法在无 GUI 环境跑**：因依赖 tauri，测试在 WSL / headless CI 上的可用性受限于 webview2 等平台依赖。

## 位置

- `src-tauri/Cargo.toml`：单 crate，`[lib] crate-type = ["staticlib", "cdylib", "rlib"]`
- `src-tauri/src/lib.rs`：`tauri::Builder` 入口 + `invoke_handler` 注册
- `src-tauri/src/commands/`：4 个 `#[tauri::command]` 模块，唯一依赖 tauri 的业务层
- `src-tauri/src/store.rs`：`AppState` 持有各 Service，被 commands 消费
- `src-tauri/src/services/`：7 个模块，**不依赖 tauri**（已验证：仅 `use crate::{error, database, services::...}`）
- `src-tauri/src/database/`：`mod.rs` + `schema.rs`，**不依赖 tauri**
- `src-tauri/src/error.rs`：`thiserror` + `serde`，**不依赖 tauri**

## 拆分方案

### 目标结构

```
agent-nexus/
├── crates/
│   └── nexus-core/          # 新 crate：不依赖 tauri
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs       # pub mod database; pub mod services; pub mod error;
│           ├── database/    # 从 src-tauri/src/database/ 迁移
│           ├── services/    # 从 src-tauri/src/services/ 迁移
│           └── error.rs     # 从 src-tauri/src/error.rs 迁移
└── src-tauri/
    ├── Cargo.toml           # 依赖 nexus-core + tauri
    └── src/
        ├── lib.rs           # tauri::Builder，commands 注册
        ├── commands/        # 留在 tauri crate（#[tauri::command]）
        ├── store.rs         # 留在 tauri crate（AppState 组装 core services）
        └── main.rs
```

### nexus-core/Cargo.toml（关键依赖）

```toml
[package]
name = "nexus-core"
version = "0.1.0"
edition = "2021"

[dependencies]
rusqlite = "0.32"           # Windows 动态链接配置同 src-tauri
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
url = "2.5"
uuid = { version = "1.11", features = ["v4"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }

[target.'cfg(windows)'.dependencies]
junction = "2.0"

[dev-dependencies]
tempfile = "3.14"
tokio = { version = "1.42", features = ["macros", "rt-multi-thread"] }
```

**不含** `tauri`、`tauri-build`。

### src-tauri/Cargo.toml

```toml
[dependencies]
nexus-core = { path = "../crates/nexus-core" }
tauri = { version = "2.8.0", features = [] }
serde = { version = "1.0", features = ["derive"] }   # commands 仍需 serde 序列化入参
# ... 其余 tauri 相关依赖
```

### 迁移步骤

1. 新建 `crates/nexus-core/`，`cargo new --lib crates/nexus-core`
2. 移动 `src-tauri/src/{error.rs, database/, services/}` 到 `crates/nexus-core/src/`
3. `nexus-core/src/lib.rs` 导出 `pub mod database; pub mod services; pub mod error;`
4. `src-tauri/src/lib.rs` 改为 `use nexus_core::{database::Database, services::...};`，删除被迁移模块的 `pub mod`
5. `src-tauri/src/commands/*.rs` 和 `store.rs` 的 `use crate::{...}` 改为 `use nexus_core::{...}`
6. 集成测试 `tests/sync_service.rs` 等：`use nexus_core::{database, services}`（不再链 tauri）
7. `pnpm-workspace.yaml` 或根 `Cargo.toml` 加入 workspace 成员（若用 cargo workspace）
8. SQLite 动态链接脚本 `scripts/with-sqlite.mjs` 的 manifest 路径需覆盖新 crate

### 收益

- **测试编译提速**：`cargo test -p nexus-core` 不再链 tauri/webview2/windows，冷编译从 >10min 降到 ~1min（仅 rusqlite + reqwest + ring）
- **死锁根治的前置条件**：core crate 独立后，可重构 `Database` 为连接池或 `tokio::sync` 友好模型，无需顾及 tauri 命令线程模型约束
- **CI 友好**：core 测试可在 Linux/WSL/headless 环境跑（仅 rusqlite bundled 即可，不需要 Windows SQLite DLL 准备脚本）
- **职责清晰**：core 是领域逻辑，src-tauri 是 IPC 适配层

### 风险与权衡

- **改动量**：纯机械迁移（移动文件 + 改 `use` 路径），无逻辑变更。预估 1-2 小时含验证。真正成本在 SQLite 脚本适配与 workspace 配置。
- **Workspace 引入**：若当前不是 cargo workspace，需新建。根 `Cargo.toml` 加 `[workspace] members = ["src-tauri", "crates/nexus-core"]`，或沿用 pnpm 混合管理（不推荐，cargo workspace 更自然）。
- **`build.rs`**：`src-tauri/build.rs` 是 `tauri-build`，core crate 不需要 build.rs。
- **`junction` 依赖**：`services/symlink.rs` 在 Windows 用 `junction` crate，core 需保留该 target-specific 依赖。

## 备注

- 本方案是 `ROADMAP.md` 第 10 行已记录的架构观察的展开。
- 本次 `add_task` 死锁是触发本 issue 的直接原因：单连接 `Mutex` 架构在方法内并发查询时必然复发，拆 core crate 是让 `Database` 重构（连接池 / `tokio::sync::Mutex` / per-service connection）可行的前置条件。
- 拆分后 `commands/` 仍留在 src-tauri，因 `#[tauri::command]` 宏依赖 tauri，且命令层本就是 IPC 适配而非领域逻辑。
- 若暂不拆，短期缓解：在 `Database` 上加 `try_connection_for()` 文档警告，或把 `add_task` 这类方法的事务+查询合并到单次 `with_conn(|conn| {...})` 闭包接口，强制单次持锁。但这只是补丁，非根治。
