# Thinking Guides

这些指南用于跨层开发前的自检。它们不是通用教程，而是 Agent Nexus 中最容易出错的思考触发器。

## 可用指南

| Guide | 何时使用 |
|-------|----------|
| [Code Reuse Thinking Guide](./code-reuse-thinking-guide.md) | 发现重复 helper、重复 query/mutation、重复 service 校验、重复路径处理时 |
| [Cross-Layer Thinking Guide](./cross-layer-thinking-guide.md) | 功能跨越 React UI、Tauri command、`nexus-core` service、SQLite schema 或 WebDAV/Provider 外部边界时 |

## Agent Nexus 特有触发器

- 改动 `Agent`、`Provider`、`Project`、`Skill`、`Prompt`、`Session`、`Distribution`、`Sync Task` 任一领域概念前，先对照 `CONTEXT.md`。
- 改动 `Project` 自定义源时，先读 ADR-0003：Skill / Prompt / Session 不强行对齐成同一种数据形态。
- 改动 Rust 测试命令或 CI 文档时，先读 `GOTCHAS.md` 和 ADR-0001，避免写入裸 `cargo test -p nexus-core`。
- 改动跨层字段时，同时检查：`crates/nexus-core` serde 类型、`src-tauri/src/commands` 暴露、`src-react/src/types`、`src-react/src/lib/api`、`src-react/src/lib/query` 和测试。
