# Design: Project Skill propagation target selection

## Scope

实现 Project custom Skill 从源 Project 传播到：

1. Global（保留现状）。
2. 其他 Project（新增）。

普通 Agent-sourced Project Skill 不新增跨 Project 传播能力。

## Current Model Summary

- `skills` 表保存 canonical Skill source。
  - `source_kind = agent`：有 `source_agent`，Agent Matrix 有一个 `source` cell。
  - `source_kind = project_custom`：无 `source_agent`，Agent Matrix 无 `source` cell。
- `skill_distributions` 当前按 `(skill_id, agent)` 记录 placement，因此只能表达一个上下文中的 Agent placements。
- 对 `project_custom` Skill，当前 `target_path_for_parts` 直接把 target 解析到 Agent 的 Global skills dir，所以 Project custom Skill 的 `cells` 实际表示 Global placements。
- 前端 `SkillRow` 已能用 `sourceless` 渲染无 source cell 的 Project source 行。

## Proposed Data Model

### Keep existing `skill_distributions` for Global placements

`skill_distributions` 继续承载：

- Global scope Skill 的 Agent placements。
- Project custom Skill propagated to Global 的 placements。
- Agent-sourced Project Skill 的 Project 内 placements（现有行为）。

为避免大迁移，先不改变其 primary key。

### Add `skill_project_distributions`

新增表用于 Project custom Skill 的跨 Project placement projection：

```sql
CREATE TABLE skill_project_distributions (
  skill_id TEXT NOT NULL,
  target_project_id TEXT NOT NULL,
  agent TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('target', 'none')),
  target_path TEXT,
  CHECK (
    (role = 'target' AND target_path IS NOT NULL)
    OR
    (role = 'none' AND target_path IS NULL)
  ),
  PRIMARY KEY (skill_id, target_project_id, agent),
  FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
  FOREIGN KEY (target_project_id) REFERENCES projects(id) ON DELETE CASCADE
);
```

约束：

- 只允许 `skills.source_kind = project_custom` 的 skill 写入该表（service 层校验）。
- `target_project_id` 不能等于 canonical source 的 `skills.project_id`。
- 不存 `source` role；目标 Project 外来 Skill 行无 Agent source cell。

## Backend API Shape

### Existing command stays

`set_skill_target(SetSkillTargetInput)` 保持不变，用于现有上下文：

- Global placement。
- 当前 Project 内 Agent-sourced Skill placement。

### New commands

新增 Rust service / Tauri commands：

```rust
SetProjectSkillTargetInput {
  skill_id: String,
  target_project_id: String,
  agent: String,
  enabled: bool,
}

SetProjectSkillProjectInput {
  skill_id: String,
  target_project_id: String,
  default_agent: String,
  enabled: bool,
}
```

语义：

- `set_project_skill_project(enabled = true)`：源侧菜单选择某个目标 Project；使用 `default_agent` 创建初始 placement。
- `set_project_skill_project(enabled = false)`：源侧取消某个目标 Project；删除该目标 Project 下此 skill 的所有 Agent placements，并将对应 rows 置为 `none` 或删除。
- `set_project_skill_target`：目标 Project 外来 Skill 行的 Agent Matrix 单 cell toggle。

`default_agent` 由前端复用 `useDefaultGlobalEntryAgent()` 传入；后端仍需 `require_agent` 校验，并校验 Agent 有 skill surface。

## Target Path Rules

新增 helper：

```rust
project_target_path_for_skill(
  target_project_path,
  canonical_path,
  agent_surface,
) -> target_project_path / agent.skill.project_dir / skill_dir_name(canonical_path)
```

规则：

- Global propagation 仍使用 `agent.skill.global_dir`。
- Cross-project propagation 使用目标 Project 的 fixed Agent project skills dir。
- 不使用目标 Project `custom_skills_dirs`。
- 目标路径存在时沿用现有 managed link 逻辑失败，不覆盖、不合并、不自动改名。

## List / Projection Model

当前 `Skill` DTO 是 canonical asset row。新增目标 Project 外来行后，需要返回“展示投影”。推荐最小兼容方案：

- 保留 `Skill.id` 为 canonical `skill_id`，新增可选字段：
  - `placementScope?: "global" | "project"`
  - `placementProjectId?: string`
  - `sourceProjectId?: string`
- 对现有 canonical rows：
  - Agent-sourced / local Project rows 行为不变。
  - Project custom source 在源 Project 中仍用 `projectId = source_project_id`。
- 对目标 Project 外来 rows：
  - `id` 可使用稳定 composite display id，例如 `${skill_id}::project::${target_project_id}`，避免 React key 冲突。
  - 新增 `canonicalSkillId` 保存真实 backend skill id。
  - `projectId = target_project_id`，使现有 Project tab / Project detail filter 能把它归到目标 Project。
  - `sourceKind = project_custom`，`sourceAgent = None`。
  - `cells` 来自 `skill_project_distributions(skill_id, target_project_id, agent)`。
  - `path` 仍为 canonical path，Open source / Reveal path 打开源 Project custom source。

前端 mutation 调用时：

- 如果 `skill.canonicalSkillId` 存在，传 canonical id。
- 目标 Project 外来 row 的 Agent Matrix 使用新 `setProjectSkillTarget` mutation。
- Global row / 源 Project row 继续使用现有 `setSkillTarget` 或新源侧传播菜单命令。

## UI Design

### Source Project custom Skill row

把当前 `PropagateToGlobal` toggle 替换为 “Propagate” 菜单：

- `Global`：显示当前 Global propagated 状态；点击未启用时用默认 Agent 创建 Global placement；点击已启用时删除 Global 所有 Agent placements（沿用当前 `unpropagateGlobal`）。
- 其他 active Project：显示是否已有 target Project placement；点击未启用时调用 `setProjectSkillProject(enabled=true)`；点击已启用时调用 `setProjectSkillProject(enabled=false)` 删除该 Project 所有 placements。

一次只操作一个目标，不做批量。

### Target Project incoming Skill row

复用当前 Global 页 Project source 行渲染逻辑：

- badge：`Project source`。
- tooltip：`Linked from Project custom source · <source Project name> · <canonical path>`。
- Agent Matrix：无 source cell，`target / none`。
- 默认 Agent 初始 target；其他 Agent 可 fan-out。
- 最后一个 target 被移除后，下一次 query data 更新中该 projection row 消失。

### Filtering and Counts

- `ProjectDetailView` 通过 `skill.projectId === dp.id` 能显示 incoming projection row。
- `SkillPage` Project tab 也通过 `projectId` 显示 incoming projection row。
- Project counts 可先基于返回的 `skills` projection rows 计数；如 `ProjectService` 自身的 `skills` 计数仍只统计 canonical rows，则本任务需要同步调整，以免列表数字和详情不一致。

## Scan / Refresh Behavior

`scan_skills` 仍只发现 canonical sources。扫描后：

- 对 `skill_distributions` 保持现有校验。
- 对 `skill_project_distributions` 校验每个 target_path 是否仍指向 canonical source；断链时回落为 `none` 或删除 row。
- 不把 symlink/junction placement 当 canonical source（现有 `discover_skill_sources` 已跳过 symlink；测试覆盖跨 Project placement）。

## Compatibility / Migration

- Schema version 从 18 升到 19。
- 新增表不迁移现有数据，现有 Global propagation 不受影响。
- 旧 rows 继续走 `skill_distributions`。
- 前端新增字段应 optional，避免 browser preview / mock payload 崩溃。

## Risks

- `Skill.id` 既要作为 React key，又要作为 command asset id；projection row 需要明确 `canonicalSkillId`，否则 mutation 会传错 id。
- `skill_distributions` 与新 `skill_project_distributions` 的 cells 语义不同，前端需要按 row context 选择 mutation。
- Project 列表 skill count 如果仍来自 DB canonical count，会与详情页 incoming row 不一致。
- 源侧取消目标 Project 必须删除该 target Project 下所有 Agent placements，否则会留下不可见残留或目标行不消失。
