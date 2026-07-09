# Project Skill Propagate 支持选择当前项目

## Goal

让 Project custom Skill 的 `Propagate to…` 目标列表不仅能选择 `Global` 与其他 Project，也能选择该 Skill 所属的当前/source Project，从而把 `Project Custom Source` 中的 canonical Skill 传播到同一 Project 的固定 Agent project skills 目录。

## User Value

- Project custom Skill 不必先跨到其他 Project 或 Global，才能落到当前 Project 的 Agent 消费端目录。
- 用户在 Project 详情页看到某个 custom source Skill 时，可以就地把它启用给当前 Project 的默认 Agent，并继续在当前 Project 内 fan-out 到其他 Agent。
- `Project Custom Source` 仍是唯一 canonical source；当前 Project 内 Agent 目录只作为 managed placement。

## Confirmed Facts

- `CONTEXT.md` 定义 `Project Custom Source` 是合法 Skill canonical source，不归属任一 Agent；Global / 其他 Agent 落点都只是 managed `Placement`，不形成新的 canonical Skill。
- 归档任务 `.trellis/tasks/archive/2026-07/07-08-project-skill-propagation-target/prd.md` 已规划并实现过 `Propagate to…` 菜单，但明确要求“菜单中包含 Global 与其他 active Project”，并把当前/source Project 排除在目标之外。
- 当前前端 `src-react/src/components/skill/propagation.ts` 的注释说明目标列表是 `Global + every other active Project`。
- 当前 `src-react/src/components/skill/propagation.ts` 在计算目标列表时执行 `if (project.id === sourceProjectId) continue;`，因此 source/current Project 不会出现在 Propagate modal 中。
- 当前后端 `crates/nexus-core/src/services/skills.rs::set_project_skill_project` 和 `set_project_skill_target` 都校验 `context.source_project_id == target_project_id` 并返回 `target project must differ from the source project`，因此即使前端传当前 Project 也会失败。
- 当前跨 Project projection 使用 `skill_project_distributions`，display row 的 `projectId = target_project_id`，source 信息通过 `sourceProjectId` / `canonicalSkillId` 保留。
- 用户已确认：“当前项目”就是 source Project 本身作为 target，用于把 Project Custom Source 传播到同一 Project 的 Agent project skills dir。

## Requirements

- Project custom Skill 的 `Propagate to…` 菜单应包含当前/source Project 作为可选 Project target。
- 选择当前 Project 时，仍然只创建 managed placement；不得把 placement 当成新的 canonical Skill。
- 当前 Project target 的初始 Agent 继续复用现有默认 entry Agent 设置。
- 当前 Project target 的目标路径应使用当前 Project 下该 Agent 的固定 project skills dir，而不是 `custom_skills_dirs`。
- 当前 Project target 应像其他 Project target 一样显示 enabled 状态、已有 target Agents，并支持取消传播。
- 当前 Project 内的 placement 管理应允许 fan-out 到其他 Agent；移除最后一个 placement 后，该 target 状态消失。
- 普通 Agent-sourced Project Skill 不新增该入口；仅 Project custom Skill 生效。
- 已存在真实目录或非托管目录时仍必须失败，不覆盖、不合并、不自动改名。
- 扫描逻辑必须继续跳过 managed symlink/junction placement，避免当前 Project placement 被识别为新的 canonical Skill。

## Acceptance Criteria

- [ ] Project custom Skill 的 `Propagate to…` modal 中出现当前 Project 条目。
- [ ] 点击当前 Project 条目会在当前 Project 默认 Agent project skills dir 创建 managed placement。
- [ ] 当前 Project 条目能显示 enabled 状态和当前 target Agent 列表。
- [ ] 再次点击/移除当前 Project target 会删除当前 Project 下该 Skill 的相关 managed placements。
- [ ] 当前 Project placement 不会覆盖已有真实 Skill 或非托管目录。
- [ ] 扫描后当前 Project placement 不会变成新的 canonical Skill。
- [ ] Global 与其他 Project 传播行为不回归。
- [ ] 后端测试覆盖 source Project 作为 target 的启用、取消、冲突保护、扫描不重复 canonical source。

## Out of Scope

- 不做普通 Agent-sourced Project Skill 的“当前 Project”传播入口。
- 不把 Project placement 提升为 canonical Skill。
- 不做双向同步或反向回流。
- 不做多选批量传播。
