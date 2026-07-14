# 架构深化机会审查

## Goal

基于近期变更热点、领域模型、ADR、现有实现与测试，识别 Agent Nexus 中值得实施的架构深化机会，并将确认后的候选项拆分为多个可独立规划、验证和归档的 Trellis 子任务，提升模块的深度、locality、leverage、可测试性与 AI 可导航性。

## Background

- 本次工作遵循 `improve-codebase-architecture` 与 `codebase-design` 的术语和原则。
- 用户明确要求不生成 HTML 报告；审查结果应直接沉淀到 Trellis 父/子任务中。
- 审查范围默认由近期 Git 热点驱动，而非对全仓库平均扫描。
- `CONTEXT.md` 是领域语言来源；`docs/adr/` 中已接受的决策不得在缺乏真实摩擦证据时被重新争论。
- 当前近期热点集中在 Project、Skill、Sync Task Group、Provider，以及 Tauri command / `nexus-core` 跨层调用路径。

## Requirements

1. 先阅读 `CONTEXT.md`、`GOTCHAS.md`、相关 ADR 与设计文档，再检查热点实现和测试。
2. 使用 **module**、**interface**、**depth**、**seam**、**adapter**、**leverage**、**locality** 描述架构问题与建议，不用“组件 / service / API / boundary”替代这些架构术语。
3. 对候选项应用 deletion test：删除现有 module 后，判断复杂度是消失还是扩散到调用方。
4. 每个候选项必须包含代码证据、当前摩擦、深化方向、测试收益、推荐强度，以及是否与 ADR 冲突。
5. 只保留有实际证据和近期收益的候选项，避免为假想变化提前引入 seam；遵循“一个 adapter 是假想 seam，两个 adapter 才是真实 seam”。
6. 将最终候选项分别创建为当前父任务下的 Trellis 子任务；每个子任务必须可独立规划、实施、验证和归档。
7. 父任务记录候选项排序、子任务映射、跨任务关系与首选推荐；依赖关系必须写入子任务工件，不能只依赖树结构表达。
8. 本次只产出架构审查与 Trellis 规划任务，不修改产品实现，不生成 HTML 报告，也不预先设计最终 interface。

## Acceptance Criteria

- [x] 审查覆盖近期变更热点，并引用具体文件或调用路径作为证据。
- [x] 每个保留候选项都通过 deletion test，且明确说明 module、interface、seam、adapter、depth、locality 与 leverage。
- [x] 每个候选项都评估现有测试面与深化后的测试收益。
- [x] ADR 冲突被明确标记；无充分摩擦证据的 ADR 决策不被重新提议。
- [x] 至少创建两个可独立验证的 Trellis 子任务，每个子任务有聚焦的 `prd.md` 和可测试验收标准。
- [x] 父任务记录 Top recommendation、任务优先级和必要的任务间顺序。
- [x] 仓库中没有新增 HTML 架构报告，也没有产品实现改动。

## Task Map

### Strong

1. `07-14-deepen-project-custom-skill-propagation` — Top recommendation。
2. `07-14-deepen-distribution-source-relocation`。
3. `07-14-deepen-sync-task-group-cache-mutations`。
4. `07-14-deepen-user-task-group-persistence`。
5. `07-14-provider-window-alignment-capability-source`。
6. `07-14-deepen-provider-display-preferences`。

### Worth exploring

7. `07-14-deepen-provider-quota-surface-projection`。
8. `07-14-deepen-background-runner-orchestration`。
9. `07-14-deepen-tray-window-lifecycle`。

完整证据、拒绝项与任务顺序见 `research/architecture-review.md`。

## Out of Scope

- 实施任何候选重构。
- 在本次审查中敲定深层 module 的最终 interface。
- 安装新程序或依赖。
- 无热点或无证据支撑的全仓库“整洁化”重构。
