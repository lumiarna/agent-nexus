# Project 自定义 Skills 目录与 Global 传播

## 问题

当前 Project Skill 只扫描每个 Agent 在仓库内的固定 skills 目录：

- `Generic Agent`: `.agents/skills`
- `Claude Code`: `.claude/skills`
- `CodeX`: `.codex/skills`
- `Copilot`: `.github/skills`
- `OpenCode`: `.opencode/skills`

这适合“某个 Agent 拥有 project skill source，再通过 Agent Matrix 传播给其他 Agent”的场景，但不支持用户在 Project 层维护一个不归属任一 Agent 的自定义 skills 目录。

期望新增能力：

- Project 可以设置自定义 Skills 目录。
- 来自该目录的 Skill 在 Project 视图中不展示 Agent Matrix。
- 用户可以勾选将这些 Project custom Skill 传播至 Global。
- Global Skill 视图能看到这些被传播上来的 Skill。
- 用户可以在 Global Skill 里继续将它传播至其他 Agent。

## Why

有些 Skill 是项目团队或用户按项目语义维护的共享能力，并不天然属于 `.codex`、`.claude`、`.github` 等任一 Agent 目录。如果强行要求先放进某个 Agent 的 project skills 目录，会带来两个问题：

- 领域语义不准：source agent 只是落点选择，不是真正的 canonical source。
- UI 误导：Project 里展示 Agent Matrix 会暗示该 Skill 已经归属于某个 Agent。

更合理的模型是：Project custom skills 目录是 canonical source；传播到 Global 或其他 Agent 时都只是 managed placement。

## What to build

为 Project 增加一组可配置的 `custom_skills_dirs`。这些目录是 Project 额外扫描源，和现有 Agent 默认 project skills 目录并存，不替代、不覆盖，也不冲突。每个目录下的真实 Skill 目录仍以 `SKILL.md` 作为识别依据，并作为 canonical source 入库。

对这些 Skill：

- `scope` 仍为 `project`，并关联 `project_id`。
- source kind 为 `project_custom`，不是 agent source。
- 扫描时排除 symlink 目录，避免把 placement 当成新的 canonical source。
- Project 详情页隐藏 Agent Matrix，改为展示 Project-level propagation 控件。
- 勾选传播至 Global 时，创建从 custom canonical path 到某个 Global Skill placement 的 managed symlink / junction。
- Global Skill 页展示该 Skill，并允许继续传播到其他 Global Agent skills 目录。

## Suggested shape

### 领域模型

不要把 Global 目录中的 symlink 当成新的 Skill source。当前 Skill 模型以 `canonical_path` 表示资产身份，symlink / junction 应继续只是 `Placement`。

建议新增 source metadata：

- `skills.source_kind`: `agent | project_custom`
- `skills.source_agent`: nullable；仅 `source_kind = agent` 时有值

也可以用独立 `skill_sources` 表表达 source 信息，但短期看新增字段成本更低。

`skill_distributions` 继续只记录 Agent placement 状态。对 `project_custom` Skill，不要求存在一个 agent `source` cell。

### 扫描规则

- 固定 Agent project skills 目录按现状扫描，source kind 为 `agent`。
- Project `custom_skills_dirs` 逐个单独扫描，source kind 为 `project_custom`。
- Custom dirs 与固定 Agent project skills dirs 都参与扫描；两者是并列来源，不存在启用 custom dirs 后隐藏或禁用默认目录的语义。
- 如果 custom dir 与某个默认 Agent skills dir 解析到同一路径，应按默认 Agent source 处理，或在保存配置时提示冲突并拒绝；不要让同一路径产生两个 source kind。
- 普通扫描仍排除 symlink 目录。
- 对已经记录的 managed Global placement，可以在扫描后校验其是否仍存在且仍指向 canonical source；不要把它 upsert 成新的 canonical Skill。

### Global 传播

Project custom Skill 传播到 Global 时，本质是创建 Global placement：

```text
Project custom canonical source
  -> Global Agent skills dir placement
  -> Other Global Agent skills dir placements
```

Global 页面展示的是同一个 canonical Skill，而不是一份新的 Global Skill 副本。

如果用户删除 Global placement，下次扫描应将对应 Agent placement 标回 `none`，但不删除 canonical Skill，除非 Project custom source 本身不再存在。

## UI 表达

### Project 详情页

来自 custom skills dir 的 Skill 行不展示 Agent Matrix。可以改为：

- 左侧：Skill name / description / `Project custom` badge
- 中间：`Propagate to Global` toggle
- 右侧：Open source / Reveal path

如果需要选择 Global 入口 Agent，`Propagate to Global` 可以展开为一个小选择器：

- `Global placement`: CodeX / Claude Code / Generic Agent / Copilot / OpenCode
- 默认值待定

### Global Skill 页

短期实用方案：Global 页仍用 Agent cells 表达每个 Global Agent 的 placement 状态，但 source badge 不显示某个 Agent，而显示一个特殊来源：

- `Project source`
- 或 `Project · <project name>`
- 或 `Custom source`

矩阵里的 Agent cell 只有 `target` / `none` 两态；没有 agent `source` cell。已被传播到 Global 的 Agent 显示为 `target`，继续点击其他 Agent 时创建更多 Global placement。

这会要求前端不要再假设 `srcAgentOf(cells)` 一定能返回某个 Agent。UI 可以改成：

- `SourceBadge` 支持非 Agent source label。
- `AgentMatrixCells` 支持 “no agent source” 模式。
- tooltip 明确写成 `Linked from Project custom source`，避免用户误解 Global 里有独立副本。

## 待定问题

### `custom_skills_dirs` 如何维护

需要明确配置入口与作用范围：

- 方案 A：Project 详情页维护单个 `custom_skills_dir`
  - 优点：符合“Project 设置自定义 Skills 目录”的需求，模型简单。
  - 缺点：如果用户想维护多个目录，需要未来扩展。

- 方案 B：Project 详情页维护多个 custom skills dirs
  - 优点：更灵活。
  - 缺点：扫描、排序、冲突处理和 UI 都更复杂。

- 方案 C：全局 Settings 提供默认 custom skills dir 模板，Project 可覆盖
  - 优点：适合统一约定，例如每个项目都用 `skills/` 或 `.nexus/skills/`。
  - 缺点：会引入 inheritance 语义，需要明确默认值变更是否影响已有 Project。

决定采用方案 B：每个 Project 可以维护多个 `custom_skills_dirs`，支持相对路径和绝对路径；相对路径按 Project root 解析。它们只作为额外 Project custom source，不影响现有 Agent 默认 skills 目录的扫描与传播。

需要补充规则：

- 每个 Project 的 custom dirs 应去重，规范化后相同路径只保留一次。
- custom dir 不应等于任一默认 Agent project skills dir；否则会和 agent source 语义冲突。
- custom dir 可以位于仓库内或仓库外；仓库外路径需要在 UI 中明确显示为 external path。
- 删除某个 custom dir 配置后，其下已扫描的 custom Skill 应在下一次扫描中失效或标记 missing，具体策略待实现时确认。

### Global Skill 的 agent source 如何显示

同意短期实用方案：Project custom Skill 在 Global 页没有 agent source。

UI 需要明确这一点：

- 不显示某个 Agent 的 source 状态。
- 行内 source badge 显示 `Project source` 或 `Project · <project name>`。
- Agent Matrix cells 仅表达 Global placements：`target` / `none`。
- 对这类 Skill 禁用“source cell 不可点击”的逻辑，因为没有 source cell。
- 如果 Global placement 缺失或断链，显示 `Missing placement` 或将对应 cell 回落为 `none`。

还需决定文案选型：

- `Project source`：短，适合表格。
- `Project · <project name>`：信息更完整，但可能占宽。
- `Custom source`：最泛化，但不够说明来自 Project。

建议 Global 列表使用 `Project source` badge，hover tooltip 显示具体 Project 名和 canonical path。

## Acceptance criteria

- [ ] Project 可以配置多个自定义 Skills 目录。
- [ ] 自定义 Skills 目录与现有 Agent 默认 project skills 目录并存，不替代、不隐藏、不冲突。
- [ ] 扫描能发现这些目录下包含 `SKILL.md` 的真实 Skill 目录。
- [ ] Project custom Skill 不展示 Agent Matrix。
- [ ] Project custom Skill 可以传播至 Global，并创建 managed placement。
- [ ] Global Skill 页能展示被传播上来的 Project custom Skill。
- [ ] Global Skill 页能继续将该 Skill 传播至其他 Agent。
- [ ] 普通扫描不会把 Global symlink placement 当成新的 canonical Skill。
- [ ] 删除或断开 placement 后，UI 能正确回落为未传播或缺失状态。
- [ ] 冲突时不覆盖已有真实目录或非托管文件。
- [ ] 测试覆盖：custom source 扫描、Global placement 创建、Global 再传播、symlink 不成为新 source、Project source missing 状态。

## Out of scope

- 不做 Project custom Skill 的 Cloud 备份策略变更。
- 不做 Global placement 提升为 canonical source。
- 不做不同 Project 中同名 custom Skill 的自动合并。
- 不做双向同步或 target 回流 source。

## Notes

这个需求会触碰现有 Agent Matrix 的核心假设：每个 Skill 行必须有一个 Agent source。实现前应先更新 `CONTEXT.md` 中 `Source Agent` / `Agent Matrix` / `Skill` 的定义，明确 Project custom source 是合法 canonical source，但不是 Agent source。
