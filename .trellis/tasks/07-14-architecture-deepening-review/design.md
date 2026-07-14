# 架构深化机会审查 — 设计

## Review Module

本父任务的 interface 是“证据驱动候选集 + Trellis 子任务树”，implementation 包括热点识别、领域/ADR 约束、Explore 审查、deletion test、历史去重与任务拆分。用户明确替换了默认 HTML 输出，因此不生成临时报告。

## Evidence Flow

1. Git 热点确定扫描范围。
2. `CONTEXT.md` 提供领域语言，ADR 限制不可随意重新争论的决定。
3. 代码、测试、现有 issue 与过去架构审查提供摩擦证据。
4. 每个候选必须通过 deletion test，并检查 seam 是否至少有两个真实 adapter。
5. 候选按独立验证面拆成子任务；父任务保存排序、依赖与最终集成审查。

## Task Tree Shape

- 父任务不承载产品实现，也不应进入 `in_progress`。
- 九个子任务分别拥有独立 module 深化目标和验收面。
- 子任务当前只完成需求种子，不提前设计最终 interface。
- 选择某个子任务实施时，先运行 grilling；如需比较 interface，按 `codebase-design` 的 design-it-twice 模式补 `design.md` 与 `implement.md`，再进入启动审查。

## Compatibility

- 不推翻 ADR-0001～0003。
- 不重复已落地的上一轮 deep module。
- 不安装依赖，不修改产品代码，不新增 HTML。
- 跨任务冲突通过 `research/architecture-review.md` 的顺序约束处理。

## Rollback

若某个候选在后续 grilling 中无法证明真实 leverage，可删除该子任务与父子链接，不影响其他候选。若发现与 ADR 冲突，先判断摩擦是否足以重开 ADR；否则关闭候选。
