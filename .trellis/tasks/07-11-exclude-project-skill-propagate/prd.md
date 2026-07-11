# 从 Sync - Project Symlinks 中排除 Project Skill Propagate

## Goal

`Sync - Project Symlinks` 仅展示未被 Agent Nexus 管理的项目目录链接；Project custom Skill 通过 `Propagate to…` 创建的 Project placement 属于系统管理的链接，不应作为手工/外部 symlink 出现在该列表中。

## Confirmed Facts

- `Project Skill Propagate` 的 Project placement 记录在 `skill_project_distributions`，目标链接路径保存在 `target_path`，并可能传播到当前/source Project 或其他 active Project。
- `Project Symlink Inventory` 已通过 `distribution::project_managed_target_identities` 隐藏普通 project-scope Agent Skill 与 Prompt 的 managed placement，但当前查询未覆盖 `skill_project_distributions`。
- 因此 Project custom Skill Propagate 创建的链接可能被 `SyncPage.tsx` 的 `Project Symlinks` 区域扫描并展示。
- 现有行为要求：无关的、未被任务或 Distribution 管理的 symlink 仍应继续展示；已有 Sync task 管理的链接也应继续隐藏。

## Requirements

- Project Symlink Inventory 必须识别 `skill_project_distributions` 中仍然存在且指向对应 canonical Project custom Skill 的 managed target，并将其从返回结果中排除。
- 该排除规则必须覆盖 Project Skill Propagate 到当前/source Project 与其他 Project 的 placement，以及 placement 的多个 target Agent。
- 只有记录与实际链接目标、canonical source 匹配时才视为 managed；失效、被替换或指向不匹配 source 的链接不得被无条件隐藏。
- 不改变 Project Skill Propagate、普通 Skill/Prompt Distribution、Sync task 或手工删除功能的既有行为。

## Acceptance Criteria

- [ ] Project custom Skill 通过 `Propagate to…` 创建的 Project placement 不出现在 `Sync - Project Symlinks` 列表中。
- [ ] 当前/source Project 与其他 Project 的 Project Skill placement 均不会出现在列表中，多个 Agent placement 也全部覆盖。
- [ ] 删除或替换 managed placement 后，实际存在且不再匹配 managed Distribution 的链接会重新出现在列表中。
- [ ] 未被任何管理关系覆盖的普通 Project symlink 仍出现在列表中。
- [ ] Rust 单元/集成测试覆盖上述隐藏、匹配校验与无关链接保留行为。

## Out of Scope

- 不修改 Project Skill Propagate 的入口、目标选择、Agent placement 路径或数据库模型。
- 不改变 `SyncPage` 的展示文案或交互。
- 不新增迁移或安装依赖。
