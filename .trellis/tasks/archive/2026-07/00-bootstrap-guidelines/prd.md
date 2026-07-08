# 中文化 Trellis Spec Bootstrap

## Goal

基于当前代码库与既有产品/架构文档，刷新 `.trellis/spec/`，让未来 AI 编码与检查任务能加载到真实、可执行、项目专属的开发规范。生成的 spec 文档优先使用中文表达，保留必要英文技术术语、代码符号、文件名、命令和领域 canonical 名称。

## Background / Confirmed Facts

- 项目根文档要求先阅读 `CONTEXT.md` 和 `GOTCHAS.md`。
- 项目结构由 `src-react/` 前端、`src-tauri/` 后端与 `docs/` 设计文档组成。
- 现有 `.trellis/spec/` 已有脚手架目录：
  - `.trellis/spec/agent-nexus/frontend/`
  - `.trellis/spec/agent-nexus/backend/`
  - `.trellis/spec/nexus-core/backend/`
  - `.trellis/spec/guides/`
- 多数现有 spec 仍包含英文模板内容、`To fill` 状态或通用占位说明，不满足真实项目规范要求。
- `CONTEXT.md` 已定义 Agent Nexus 的核心领域术语；这些术语的 canonical English 名称（如 `Agent`、`Provider`、`Project`、`Skill`、`Prompt`、`Session`、`Distribution`、`Cloud`）应在中文 spec 中保留或中英并列，避免误译。

## Scope

- Spec directory:
  - `.trellis/spec/agent-nexus/frontend/`
  - `.trellis/spec/agent-nexus/backend/`
  - `.trellis/spec/nexus-core/backend/`
  - `.trellis/spec/guides/`（仅在与项目实际不符或需补充本地经验时更新）
- Source directories / docs to inspect:
  - `src-react/`
  - `src-tauri/`
  - `docs/design/`
  - `docs/adr/`
  - `CONTEXT.md`
  - `GOTCHAS.md`
  - `CLAUDE.md`
- Out of scope:
  - 不修改产品源代码，除非发现 spec 无法准确描述现状且用户另行授权。
  - 不引入新的架构约束或重构建议作为“规范”；bootstrap 只记录当前真实做法与已确认约束。
  - 不把英文技术术语、Rust/TypeScript 标识符、命令、路径强行翻译。

## Requirements

- R1: 重写或合并现有模板 spec，使其成为项目真实规范，而不是通用框架建议。
- R2: 每条重要规则必须由真实来源支撑：代码文件、测试文件、设计文档、ADR、`CONTEXT.md` 或 `GOTCHAS.md`。
- R3: spec 文档主体优先使用中文；术语、canonical domain names、代码符号、路径、命令保持英文或原文。
- R4: 更新所有相关 `index.md`，确保索引、文件清单与最终 spec 文件集一致。
- R5: 删除或改写模板占位内容，包括 `To fill`、`TBD`、`placeholder`、“How to fill”等脚手架说明。
- R6: 明确记录项目特有 anti-pattern，例如 `CONTEXT.md` 中的 `_Avoid_` 语义、`GOTCHAS.md` 中 Windows / SQLite 测试陷阱。
- R7: Spec 应帮助未来子代理完成编码和检查：包含适用场景、本地模式、参考文件、常见错误、可靠验证命令。

## Acceptance Criteria

- [ ] `.trellis/spec/` 中目标目录不再保留模板占位说明、`To fill` 状态或空泛通用建议。
- [ ] 每个目标 spec 文件至少包含可追溯到真实文件或设计文档的项目规则。
- [ ] Spec 主体语言为中文，英文仅用于术语、代码、路径、命令、canonical 名称或必要引用。
- [ ] `index.md` 文件与实际 spec 文件完全匹配，并使用中文说明各文件用途。
- [ ] 与领域术语相关的规则符合 `CONTEXT.md`，避免把实现层短 ID 当作领域展示名。
- [ ] 与测试/验证相关的规则覆盖 `GOTCHAS.md` 中的 Windows SQLite 注意事项。
- [ ] 运行最终检查时，`grep -R "To fill\|TODO: fill\|placeholder\|TBD" .trellis/spec` 不应发现未处理占位内容（历史引用除外需注明原因）。

## Open Questions

当前没有阻塞性问题；若实施中发现某个目录缺少足够代码证据，应在执行笔记中记录并按现状收敛 spec 范围。
