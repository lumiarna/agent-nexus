# nexus-core Database Guidelines

## 适用范围

适用于 `crates/nexus-core/src/database/` 与 service 中的 rusqlite 访问。

## 基础模式

- 使用 rusqlite 直写 SQL，不使用 ORM。依据：`docs/design/Architecture Design.md`、`docs/design/Database Schema.md`。
- `Database` 持有 `Mutex<Connection>`，入口为 `open` / `open_in_memory` / `connection`。参考 `database/mod.rs`。
- 初始化时启用 `foreign_keys` 并运行 `schema::migrate`。
- 测试优先使用 `Database::open_in_memory()`，参考 `tests/sync_service.rs`、`tests/project_service.rs`。

## Migration 规则

- 当前 schema 版本由 `database/schema.rs` 的 `CURRENT_SCHEMA_VERSION` 管理。
- 新迁移按顺序新增 `migrate_to_vN`，并在 `migrate` 中串联；不要修改历史迁移含义来“修复”已发布版本。
- 项目尚未上线时，`GOTCHAS.md` 允许数据库迁移以最小成本实现，但仍要保持启动迁移路径可运行。
- Windows SQLite 动态链接依据 ADR-0001；测试命令不要绕过 `with-sqlite` 包装。

## 事务与不变量

- 涉及多表写入或“先文件系统 placement 后 DB”的操作要有回滚思路。参考 `services/distribution.rs::write_target` 和 `services/sync/task_lifecycle.rs` 创建 link placement 失败回滚。
- `Agent Matrix` 每个 agent 集合完整、source 唯一等不变量由 service 维护，不能只依赖 UI 或 partial unique index。
- `Sync Task` 的 `direction` 由 `source_type` / `target_type` 派生；`Cloud→Cloud` 非法；`Symlink` / `Junction` 仅限 Distribution。

## 表和字段命名

- 表名和字段名对齐 `CONTEXT.md` 领域词：`projects`、`skills`、`prompts`、`providers`、`session_index`、`task_groups`、`tasks`、`skill_distributions`。
- 时间戳使用 Unix epoch seconds；主键多数为 UUID TEXT。
- JSON/text list 字段只用于弱结构或简单配置，例如 `connection_params`、换行分隔的 `custom_skills_dirs` / `extra_prompt_files`。

## 常见错误 / anti-pattern

- 裸写 SQL 时忘记 `ON DELETE CASCADE` 或 service 级完整性检查。
- 将 Session 正文写入数据库；设计文档要求 session_index 只存元数据和摘要，正文留文件系统/Cloud。
- 把 `Project Path` 当稳定身份；稳定身份是 `Project Key`。
