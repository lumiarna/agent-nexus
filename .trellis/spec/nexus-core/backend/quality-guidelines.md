# nexus-core Quality Guidelines

## 必守领域约束

- `CONTEXT.md` 是领域真相源。不要把 `Agent` 误建模为 model/provider/account；不要把 `OpenCode Go` 当 `Agent`；不要把 `Provider quota` 做 project-level 归因。
- `Distributable Asset` 主要是 `Skill` / `Prompt`；`Session` 是 `Archivable Content`，不进入 Agent Matrix。
- `Distribution` 是 Local→Local 单向关系，不是双向同步；`Sync Task` 永远 `1 source → 1 target`。
- `Project Key` 是稳定身份，`Project Path` 可变；扫描发现同 key 新路径应续认同一 Project。

## 测试模式

- 服务级集成测试放在 `crates/nexus-core/tests/`，按领域文件组织。
- 使用 `tempfile::TempDir` 创建 Git repo / 文件系统 fixture；参考 `tests/sync_service.rs` 的 `git_repo` helper。
- Windows symlink 权限特殊：测试中目录链接使用 `#[cfg(windows)] Junction`、非 Windows 使用 `Symlink`，参考 `tests/sync_service.rs`。
- Provider / WebDAV 等外部边界用 fake server / fake adapter，不让核心测试依赖真实网络。

## 验证命令

- Windows 下不要裸跑 `cargo test -p nexus-core`；遵守 `GOTCHAS.md`：

```bash
pnpm rust:test
# 或
node scripts/with-sqlite.mjs cargo test -p nexus-core
```

- 单文件格式修复可用：`cargo fmt --all -- <path>`；最终格式验证再跑 `pnpm rust:fmt`。

## 代码组织质量

- 重复 helper 上提到 `services/util.rs`、`paths.rs`、`system_open.rs`。
- 深模块优先：Distribution、Provider quota derive、cron、Transfer seam 已是本项目模式，不要在外层散落重复规则。
- 新增外部副作用时，只在必要边界加轻量 port；不要全面六边形化，也不要为 `Database` 造 trait。

## 常见错误 / anti-pattern

- 把 UI 便利模型写入领域模型，例如把多 target form 直接落成一个 multi-target task。
- 把 Prompt extra file 建成新的 `Project Prompt Custom Source`；ADR-0003 明确它继承 Agent namespace。
- 把 Session Directory 改成 Vec；ADR-0003 明确 Session 始终单值。
