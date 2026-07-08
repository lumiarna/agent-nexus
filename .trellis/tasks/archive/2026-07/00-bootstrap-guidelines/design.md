# Design: 中文化 Trellis Spec Bootstrap

## Architecture and Boundaries

本任务只修改 `.trellis/spec/` 与任务规划文件，不修改 `src-react/`、`src-tauri/` 或数据库迁移等产品代码。

Spec 结构应跟随现有代码边界：

- `agent-nexus/frontend`: React + Vite + TypeScript 前端约定。
- `agent-nexus/backend`: Tauri 边界、命令暴露、应用集成与后端编排约定。
- `nexus-core/backend`: Rust core、SQLite、领域服务、仓储与测试约定。
- `guides`: 跨层思考与代码复用指南；默认保留，必要时补充 Agent Nexus 特有触发条件。

## Data / Knowledge Sources

优先级从高到低：

1. `CONTEXT.md`：领域模型、canonical terminology、avoid list。
2. `GOTCHAS.md`：本地开发与验证陷阱。
3. `docs/design/` 与 `docs/adr/`：架构和产品约束。
4. `src-react/`、`src-tauri/`、测试与脚本：实际代码模式。
5. 现有 `.trellis/spec/`：只作为待替换脚手架，不作为事实来源。

## Language Contract

- 默认中文叙述。
- 保留英文术语：Rust / TypeScript / React / Tauri / SQLite / WebDAV / Provider / Agent / Skill 等。
- 保留 canonical domain names：`Agent`、`Provider`、`Project`、`Skill`、`Prompt`、`Session`、`Distribution`、`Cloud` 等。
- 文件路径、命令、标识符、枚举值、数据库表/字段名不翻译。
- 如中文可能造成歧义，采用“中文解释 + 英文原词”的格式。

## Spec Writing Pattern

每个具体 spec 文件尽量采用以下结构：

1. 适用范围。
2. 本项目采用的模式。
3. 参考文件或文档证据。
4. 常见错误 / anti-pattern。
5. 验证方式（如有可靠命令）。

## Trade-offs

- 不追求覆盖所有文件；优先覆盖未来编码最容易出错的边界、数据流、命名、测试与验证约束。
- 不把期望中的重构写成规范；若发现代码有不一致，只记录“现状”和“新增代码优先跟随的主流模式”。
- 不按模板强行保留文件；可以合并、重命名或删除 spec 文件，只要 `index.md` 同步更新。

## Rollback

所有改动集中在 `.trellis/spec/` 与 `.trellis/tasks/07-08-trellis-spec-bootstrap-zh/`。如需回滚，可按目录级别还原，不影响产品运行。