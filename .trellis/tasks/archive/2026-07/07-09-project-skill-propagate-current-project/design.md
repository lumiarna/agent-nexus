# Design: Project Skill Propagate 支持选择当前项目

## Scope

在既有 Project custom Skill `Propagate to…` 模型上，允许 source/current Project 自身作为 Project target。

本任务不是重新设计 Project Skill 传播，而是在现有 `skill_project_distributions` projection 模型中放宽 `target_project_id == source_project_id` 的限制，并确保扫描、列表、UI 状态不会把当前 Project placement 误识别为新的 canonical Skill。

## Current Model Summary

- Project custom Skill 的 canonical source 来自 Project `custom_skills_dirs`，`source_kind = project_custom`，无 Source Agent。
- `skill_distributions` 表继续用于 Global placements。
- `skill_project_distributions` 表用于 Project target placements，按 `(skill_id, target_project_id, agent)` 记录 target Agent placement。
- `list_skills` 会为有 live target 的 `(skill_id, target_project_id)` 追加 projection row：
  - `id = {skill_id}::project::{target_project_id}`，作为 display key。
  - `canonicalSkillId = skill_id`，mutation 必须传 canonical id。
  - `projectId = target_project_id`。
  - `sourceProjectId` 保留 canonical source Project。
  - cells 来自 `skill_project_distributions`，无 source cell。
- 当前前端 `computePropagationTargets` 排除 source Project。
- 当前后端 `set_project_skill_project` / `set_project_skill_target` 禁止 source Project 作为 target。

## Proposed Change

### Frontend target list

`computePropagationTargets` 不再排除 `project.id === sourceProjectId`。

目标列表语义变为：

- `Global`。
- 每个 active Project，包括 source/current Project。

当前 Project 条目与其他 Project 条目使用同一 `PropagationTarget` shape：

- `projectId = sourceProjectId`。
- `projectName` 使用 Project name。
- `enabled` 通过匹配 incoming/projection row 的 `canonicalSkillId + placementScope + placementProjectId` 得到。
- `defaultAgent` 仍复用现有默认 Global entry Agent 设置。
- `targetAgents` 来自 projection row cells。

### Backend validation

`set_project_skill_project` 和 `set_project_skill_target` 移除“target project must differ from source project”的校验。

保留其他校验：

- Skill 必须存在。
- Skill 必须是 `project_custom`。
- Skill 必须有 source project。
- target Project 必须存在且 active（沿用现有 `project_root` / context helper 约束）。
- target Agent 必须是 skill-capable Agent。
- target path 已存在真实目录或非托管目录时失败，不覆盖、不合并、不自动改名。

### Target path rules

当前 Project target 与其他 Project target 使用同一 helper：

```text
project_target_path_for_skill(
  target_project_path,
  canonical_path,
  target_agent,
)
```

因此：

- target Project 为 source Project 时，目标仍是该 Project 下目标 Agent 的固定 project skills dir。
- 目标绝不落入 `custom_skills_dirs`。
- canonical path 仍是 Project Custom Source 中的原始 path。

### Projection and UI state

允许 `target_project_id == source_project_id` 后，projection row 的 `projectId` 与 canonical source row 的 `projectId` 可能相同，因此同一 Project 详情页中可能同时存在两类 row：

1. Canonical source row：`sourceKind = project_custom`，无 `canonicalSkillId`，显示 `Propagate to…` control。
2. Current Project placement projection row：`canonicalSkillId = source skill id`，`placementScope = project`，`placementProjectId = sourceProjectId`，显示 sourceless Agent Matrix。

前端必须继续通过 `isIncoming` / `canonicalSkillId` / projection fields 区分二者：

- canonical row 用于 source-side Propagate menu。
- projection row 用于 Agent Matrix fan-out / per-Agent removal。

这保持“source 与 placement 是两个展示投影”的现有设计，不把 placement 提升为 canonical Skill。

### Scan behavior

扫描仍只发现 canonical sources：

- `discover_skill_sources` 已跳过 symlink/junction 目录。
- 当前 Project placement 是 managed link，也必须被跳过。
- 扫描后 canonical source row 仍只有一条；current Project projection row 来自 `skill_project_distributions`，不是扫描新增的 `skills` row。

新增或调整测试应覆盖：

- source Project placement 被扫描跳过。
- rescan 后不会出现重复 canonical source。

## Compatibility

- 不新增 schema migration；复用已存在的 `skill_project_distributions`。
- Global propagation 行为不变。
- 其他 Project target 行为不变。
- 普通 Agent-sourced Project Skill 不受影响，因为 propagation menu 仍由 `isProjectCustomSkill` gated。

## Risks

- 同一 Project 详情页同时出现 canonical row 与 projection row，可能造成 UI 计数或 key 混淆；必须依赖 composite display id 和 `canonicalSkillId` 区分。
- 如果 mutation 对 projection row 误传 display id，会失败；现有 projection 逻辑应继续使用 canonical id。
- 若扫描跳过 managed symlink 的逻辑回归，当前 Project placement 最容易被误认为同 Project 下的新 canonical source；测试需显式覆盖。

## Rollback

- 前端可恢复 `if (project.id === sourceProjectId) continue;`。
- 后端可恢复 source/target 不同校验。
- 不涉及数据库结构变更，回滚低风险。
