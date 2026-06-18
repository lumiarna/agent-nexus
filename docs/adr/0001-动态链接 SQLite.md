# 动态链接 SQLite

Windows 公司开发机上的安全软件会对 `rusqlite` 的 `bundled` SQLite 静态代码产生启发式误报，使部分小型未签名 Rust 测试二进制在 `CreateProcess` 阶段返回 `Access is denied (os error 5)`。我们决定保留 `rusqlite` 作为 SQLite 访问层，但在 Windows 开发、测试和打包链路中改为动态链接官方 SQLite DLL/import library，而不是继续静态编译 SQLite C 源码。这样可以保留现有 SQL 数据模型和代码结构，同时让本机 Rust 测试进入真实业务断言；代价是必须显式管理 `sqlite3.dll`、`sqlite3.lib` 的版本、校验、CI 缓存和 Tauri 分发。

## Considered Options

- 继续使用 `rusqlite` 的 `bundled` 特性：分发最简单、构建最自包含，但已在当前 Windows 开发机上稳定触发安全软件误报，阻断 TDD 工作流。
- 替换为 SQLx 或 SeaORM：不能从根上解决问题，因为 SQLite 驱动仍可能走 bundled SQLite；同时会引入 async/ORM 迁移成本。
- 替换为 Diesel：默认动态链接 SQLite，更可能避开误报，但会引入 schema DSL、宏和迁移体系，对当前本地小数据库偏重。
- 替换为纯 Rust 存储：可避开 SQLite C 代码，但会放弃 SQL、关系表和现有迁移模型，改动面过大。
- 保留 `rusqlite` 并动态链接官方 SQLite：代码改动最小，实验已验证能消除 `os error 5`，因此选用该方案。

## Consequences

- Windows 构建环境必须提供与 `rusqlite` 兼容的官方 SQLite x64 DLL 和 MSVC import library；仅有 `sqlite3.dll` 不够，链接阶段还需要 `sqlite3.lib`。
- CI 和本地开发需要固定 SQLite 版本并校验来源，不能依赖系统或其他软件目录中偶然存在的 DLL。
- Tauri 打包时必须把 `sqlite3.dll` 随应用分发或放到运行时可解析的位置。
- 动态链接解决的是安全软件误报；Windows symlink 权限导致的 `os error 1314` 是独立问题，需要单独处理。

## Implementation Notes

- Windows 通过 `scripts/with-sqlite-windows.ps1` 固定下载 `sqlite-dll-win-x64-3530200.zip`，校验 SHA-256 `5D40DE68DA94CEE0FBB01A7CAAE96C9226872549FB007E826F63CD7BB464B463` 后用 MSVC `lib.exe` 从 `sqlite3.def` 生成 `sqlite3.lib`。
- `pnpm tauri ...` 和 `pnpm rust:test` 通过 `scripts/with-sqlite.mjs` 包装，在 Windows 下设置 `SQLITE3_LIB_DIR` 与 DLL `PATH`；非 Windows 直接透传原命令。
- `src-tauri/Cargo.toml` 仅在 Windows 使用动态 `rusqlite`，非 Windows 继续使用 bundled，避免把 Windows 公司机约束扩散到其他平台。
