# nexus-core Backend 规范

本目录描述 `crates/nexus-core/` 的 Rust 领域层规范。`nexus-core` 是无 Tauri 依赖的 core crate，承载数据库、领域 service、ports/adapters 与测试。

## 规范清单

| Guide | 用途 |
|-------|------|
| [Directory Structure](./directory-structure.md) | core crate 模块边界、service 组织与 deep module 模式 |
| [Database Guidelines](./database-guidelines.md) | rusqlite、schema migration、事务和表命名规则 |
| [Error Handling](./error-handling.md) | `AppError` / `AppResult`、validation 与 IO/DB 错误传播 |
| [Quality Guidelines](./quality-guidelines.md) | 测试、验证命令、Windows SQLite 陷阱、领域 anti-pattern |
| [Logging Guidelines](./logging-guidelines.md) | request log、外部请求、secret 脱敏和后台错误记录 |

## 必读依据

- `CONTEXT.md`：领域语言与禁止误建模的 `_Avoid_`。
- `docs/design/Architecture Design.md`：`nexus-core` / Tauri shell 分离、deep module、ports & adapters。
- `docs/design/Database Schema.md` 与 `crates/nexus-core/src/database/schema.rs`：持久化设计与当前 schema。
- `docs/adr/0001-动态链接 SQLite.md`：Windows SQLite 动态链接决策。
- `docs/adr/0003-asset-custom-sources.md`：Skill / Prompt / Session 自定义源不要强行对齐。
