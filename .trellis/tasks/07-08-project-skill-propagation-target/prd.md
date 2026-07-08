# Project Skill propagation target selection

## Goal

让 Project custom Skill 的传播入口可以选择目标范围：继续传播到 Global，或传播到其他 Project。传播到其他 Project 后，目标 Project 应像当前 Global 页展示已传播的 Project source Skill 一样显示该 Skill 行，并允许在目标 Project 内用 Agent Matrix 管理各 Agent placement。

## User Value

- 项目级共享 Skill 可以直接复用到其他 Project，不必先进入 Global 再人工处理。
- 源 Project 可以集中看到并取消对 Global / 其他 Project 的传播关系。
- 目标 Project 能看到外来 Skill，并像当前 Global 页一样逐 Agent fan-out 或移除 placement。

## Confirmed Facts

- `CONTEXT.md` 定义：`Skill` 是可传播资产；`Project Custom Source` 是合法 canonical source，但没有 `Source Agent`；Global 或其他 Agent 落点都只是 managed `Placement`，不形成新的 canonical Skill。
- `docs/issues/archive/260627-1145-project-custom-skills-dir-global-propagation.md` 定义了当前模型：Project custom Skill 在 Project 详情页隐藏 Agent Matrix，使用 `Propagate to Global` 控件；Global Skill 页展示这些 Project source 并允许继续 fan-out 到其他 Global Agent。
- ADR-0003 明确 Skill 的 `customSkillsDirs: Vec<String>` 是 Project 级 canonical source；不要把 Skill/Prompt/Session 抽象成统一 custom source 形态。
- 当前 `src-react/src/components/skill/SkillRow.tsx` 中，Project custom Skill 在 Project view 使用单一 `PropagateToGlobal` toggle；Global view 通过同一 `SkillRow` 展示 Project source 行。
- 当前 Project source 行的来源显示是：badge `Project source`；tooltip `Linked from Project custom source · <Project name> · <canonical path>`。
- 当前 `src-react/src/components/project/ProjectDetailView.tsx` 的 `propagateGlobal` 调用 `setSkillTarget(skillId, entryAgent, true)`，文案为 `Propagated to Global · <Agent>`。
- 当前 `crates/nexus-core/src/services/skills.rs` 中，`source_kind = project_custom` 的 target path 始终解析为目标 Agent 的 Global skills dir，因此 `set_skill_target` 只能产生 Global placement。
- 当前 `crates/nexus-core/src/services/skills.rs::discover_skill_sources` 跳过 symlink/junction 目录，因此 managed placement 不会被扫描成新的 canonical Skill。
- 当前 `crates/nexus-core/src/services/symlink.rs` 的 managed link 创建在目标路径已存在时失败，不覆盖已有真实目录或非托管文件。
- 现有测试 `crates/nexus-core/tests/skill_service.rs` 覆盖 Project custom Skill 传播到 Global、Global fan-out、rescan 不把 Global symlink 当 canonical source、placement 删除后回落。

## Requirements

- Project custom Skill 的传播入口应允许用户选择目标类型：Global 或其他 Project。
- 本次跨 Project 传播仅支持 Project custom Skill；普通 Agent-sourced Project Skill 不在本次范围内，继续保持现有 Project 内 Agent Matrix 行为。
- 传播交互采用一次选择一个目标的菜单，不做多选批量传播；菜单中包含 Global 与其他 active Project，已同步目标应显示当前状态并可取消。
- 选择 Global 时，应保留现有能力：创建 Global Agent skills dir placement，并允许在 Global Skill 页继续 fan-out。
- 选择其他 Project 时，用户只选择目标 Project；目标 Agent 使用现有 Settings 中用于 Project custom Skill Propagate to Global 的默认入口 Agent，不新增专用设置。
- 目标落点应是目标 Project 下默认 Agent 的固定 project skills 目录，例如默认 Agent 为 `Claude Code` 时落到目标项目 `.claude/skills/<skill>`。
- 不把 placement 落到目标 Project 的 `customSkillsDirs`，避免被误识别为新的 canonical source。
- 选择其他 Project 时，必须保持原 Skill 的 canonical source 不变；目标 Project 内产生的内容只能是 managed placement，不能变成新的 Project custom canonical Skill。
- 传播到其他 Project 后，目标 Project 的 Skill 表需要像 Global 页展示已传播的 Project source Skill 一样显示一条外来 Skill 行，并拥有自己的 Agent Matrix。
- 目标 Project 外来 Skill 行的 Agent Matrix 不显示 `source` cell；默认 Agent 初始为 `target`，其他 Agent 初始为 `none`，可继续 fan-out 为 `target`。
- 目标 Project 外来 Skill 行允许用户像当前 Global 页 Project source 行一样逐 Agent 取消 placement；当最后一个 Agent placement 被移除后，该外来 Skill 行自动消失。
- 源 Project 的传播控件需要支持取消同步某个目标 Project；取消逻辑应类似当前取消 Global 传播：删除该目标 Project 内该 Skill 的所有 Agent placements，并让目标 Project 外来 Skill 行消失。
- 如果目标 Project 的默认 Agent skills 目录里已存在同名真实 Skill 或非托管目录，传播应失败并提示目标已存在；不覆盖、不合并、不自动改名。
- UI 文案必须清楚区分 `Project source`、`Global placement`、`Project placement`，避免暗示复制或双向同步；外来 Skill 行的 source badge/tooltip 沿用当前 Global 页 Project source 行现状。
- 扫描逻辑必须避免把跨 Project placement 识别成新的 canonical source。

## Acceptance Criteria

- [ ] Project custom Skill 行可以打开传播目标菜单，菜单包含 Global 与其他 active Project。
- [ ] 选择 Global 的行为与现有 Propagate to Global 能力兼容，Global 页仍可继续 fan-out 到其他 Agent。
- [ ] 选择其他 Project 时，交互只要求选择 Project，不要求逐次选择 Agent；目标 Agent 由现有 Settings 默认值决定。
- [ ] 选择其他 Project 后，目标 Project 默认 Agent project skills 目录中创建 managed placement。
- [ ] 目标 Project 的 Skill 表会出现该外来 Skill 行，并提供无 source cell 的 Agent Matrix。
- [ ] 在目标 Project 外来 Skill 行中逐 Agent 添加/移除 placement 可管理目标 Project 内各 Agent project skills dir。
- [ ] 在目标 Project 外来 Skill 行中逐 Agent 移除 placement 后，当最后一个 placement 被移除，该行自动消失。
- [ ] 从源 Project 取消某个目标 Project 时，会删除该目标 Project 内该 Skill 的所有 Agent placements，目标 Project 外来 Skill 行消失。
- [ ] 目标 Project placement 不会在扫描后变成新的 canonical Skill。
- [ ] 如果目标路径已存在真实 Skill 或非托管目录，传播失败且不覆盖现有内容。
- [ ] 普通 Agent-sourced Project Skill 不出现跨 Project 传播入口。
- [ ] 后端测试覆盖 Global 传播不回归、跨 Project placement 创建、目标 Project fan-out、rescan 去重/不误识别 canonical source、冲突不覆盖、取消目标 Project 清理全部 placements。

## Out of Scope

- 不做双向同步。
- 不做 target Project 对 source Project 的反向回流。
- 不把 Global 或 Project placement 提升为 canonical Skill。
- 不做不同 Project 中同名 Skill 的自动合并。
- 不支持普通 Agent-sourced Project Skill 跨 Project 传播。
- 不做多选批量传播。
- 不新增 Project-to-Project 专用默认 Agent 设置。
