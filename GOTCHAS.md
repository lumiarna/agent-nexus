- `tauri.conf.json` 已限制 `minWidth: 1100` / `minHeight: 720`，无需考虑小屏适配
- UI 允许超前，后端如果还未实现，交互时只提示/无反应均可，但不要删除或隐藏相关 UI 元素
- 由于产品还未上线，数据库迁移请尽量以最小成本实现
- 遇到问题时优先参考 `${ROOT}/Sample/cc-switch` 中的成熟实现
- Windows 下跑 Rust 测试不要直接 `cargo test -p nexus-core`，要用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`，否则可能因未设置 SQLite import library/DLL 路径而报 `LINK : fatal error LNK1181: cannot open input file 'sqlite3.lib'`
- with-sqlite 包装脚本在 `Copy-RuntimeDll` 阶段把 `sqlite3.dll` 拷到 `target/release` 时被正在运行的

  旧 `agent-nexus`（PID 41896）占用而整体失败，`cargo check` 根本没跑。绕过：手动设

  `SQLITE3_LIB_DIR=src-tauri\vendor\sqlite\windows-x64\lib` + 把 `...\bin` 加进 PATH，直接跑

  cargo，不触发 release DLL 拷贝，避免动用户的运行进程。
- 单文件修复时优先用 cargo fmt --all -- <path>，验证时再跑 pnpm rust:fmt

