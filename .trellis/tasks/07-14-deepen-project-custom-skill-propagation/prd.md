# 深化 Project custom Skill 传播模块

## Goal

让 Project custom Skill 的 canonical source、incoming projection、传播状态与用户动作形成一个更深的 module，使页面调用者通过较小 interface 获得更高 leverage，并把身份判断、状态转换和验证集中到同一 locality。

**推荐强度：Strong（架构审查 Top recommendation）**

## Background

- 当前 `Skill` 读取形态通过 `canonicalSkillId`、`placementScope`、`placementProjectId` 等可选字段表达 canonical / incoming 差异，非法组合可以被表示。
- `SkillPage` 与 `ProjectDetailView` 重复解释身份、计算传播目标并编排写入；Global 撤销还会循环执行 cell-level mutation。
- `crates/nexus-core/src/services/skills.rs:202-318` 手工拼装 projection row，`:582-749` 承担跨 Project 传播与撤销。
- `src-react/src/components/skill/SkillPage.tsx:118-222` 与 `src-react/src/components/project/ProjectDetailView.tsx:144-239` 存在平行传播编排。
- `src-react/src/components/skill/propagation.ts` 只抽出 target join；删除后复杂度会回到页面，当前 module 仍 shallow。
- `df9306a`、`db018a7`、`53bd3e5`、`997ca77` 的连续变更表明该路径 locality 不足。

## Requirements

1. 仅深化 Project custom Skill，不把 Prompt extras 或 Session Directory 纳入共同 seam；遵守 ADR-0003。
2. canonical Skill 与 incoming projection 通过 Rust serde enum / TypeScript discriminated union 显式区分，非法 identity 组合不可表示。
3. incoming Project Skill 继续作为独立行显示，并保持现有 Agent Matrix、传播 modal 与用户操作语义；本任务不重做信息架构或视觉布局。
4. core 返回 eager read model：Project custom canonical row携带 Global 与全部可传播 Project 的 destination states；每个 destination 以唯一一份 Placement cells 表达状态，不能同时返回可互相矛盾的 `globalCells` 与派生 target facts。页面不得 join Skills、Projects 与 Settings 来重建传播状态。
5. 页面不再自行解释 `canonicalSkillId ?? id`、`placementProjectId`、canonical/projection 差异或默认入口 Agent。
6. 读取与写入 interface 一并深化。单一 typed intent command 覆盖：
   - 启用或撤销某个 Global / Project target；
   - 切换某个 destination 内的 Agent placement。
7. 一次用户意图由 `nexus-core` 完整编排；Global / Project 全量撤销不得由页面循环 mutation。
8. Settings 默认入口 Agent 在执行时由后端解析并重新校验；写入 intent 不接受 `defaultAgent`。
9. 旧 `set_project_skill_project`、`set_project_skill_target` 及 Project custom 分支下的 `set_skill_target` 直接移除，不保留兼容 adapter；`list_skills` / `scan_skills` 名称保持不变。
10. 多 placement 写入采用同进程补偿式原子性：
    - 写入前完成全部领域与路径预检；
    - 文件系统步骤全部成功后才提交 Distribution 数据；
    - 任一步失败时逆序补偿；
    - 不允许 best-effort 部分成功成为正常结果。
11. 补偿失败返回独立的 reconciliation error，并在失败发生后持久化诊断 evidence；不为进程崩溃或强制退出预写 operation journal，也不增加启动/scan 恢复流程。
12. reconciliation 不新增 Repair UI / command。同一 intent 必须可幂等重试：已正确存在或已缺失的 Placement 视为已完成步骤，并继续收敛剩余状态。
13. 同一个私有 mutation lock 必须覆盖 Project custom intent、`scan_skills` 的 scan/reconcile 写阶段及其他会改写 Skill / Distribution 同一事实的 mutation，避免 plan→filesystem→transaction 期间并发漂移。
14. 可传播 Project 必须使用 Project 的有效状态：数据库标记 active 且当前 Project Path 可解析、存在。stale / hidden / missing path Project 不进入 eager targets，intent 也必须拒绝。
15. 所有保留的数据 mutation（普通 Agent target、source relocation、DMI、Project custom intent）返回权威完整 Skill catalog；React Query 统一整体替换 Skill cache，Project custom intent 还需失效受 incoming row 数量影响的 Project query。
16. 不建立公开 opaque handle、plugin registry、通用 `AssetPropagation` port 或只有一个生产 adapter 的假想 seam。
17. module interface 不返回按钮文案、toast、CSS 或 path preview 等 UI 事实。
18. 项目未上线，无需兼容旧 wire shape；实现采用 design-it-twice 推荐混合：极小 core interface + core-owned eager read model + typed destination / intent + 窄前端 mutation module。

## Acceptance Criteria

- [ ] `Skill` 跨层类型明确区分 agent canonical、Project custom canonical 与 Project custom incoming 三种 row，且每种 row 都有稳定 `rowKey` 与 canonical `skillId`。
- [ ] Rust/JSON/TypeScript contract tests 证明 enum variant 字段使用 camelCase，Agent cells 与 Placement cells 使用不同 role 类型。
- [ ] Project custom canonical / incoming row 的 cells 在类型和 core 测试中均不允许 `source`。
- [ ] Project custom canonical row一次返回 Global 与所有可传播 Project destination states，包括健康的 source Project 自身；Global 状态只表示一次。
- [ ] stale、hidden 或 Project Path 不存在的 Project 不出现在 eager targets，相关 intent 返回 validation error 且不创建目录。
- [ ] SkillPage 与 ProjectDetailView 不再重复 Project custom Skill 传播编排，也不读取默认入口 Agent来构造写入参数。
- [ ] 调用者不再使用 `canonicalSkillId ?? id`、`placementScope` 或 `placementProjectId` 发起动作。
- [ ] 旧 Project custom Skill 写入 commands、前端 payload、query hooks 和 `components/skill/propagation.ts` 被移除。
- [ ] 一次撤销可原子移除某个 Global / Project destination 的全部 placements；中途失败时数据库保持原状态，已执行文件步骤被逆序补偿。
- [ ] 补偿失败返回 reconciliation error、持久化 evidence；再次提交同一 intent 可以根据实际 Placement 状态继续收敛。
- [ ] core interface 测试覆盖 Global、source Project、跨 Project、Agent fan-out、末位删除、幂等重试、路径冲突、文件失败、数据库失败与补偿失败。
- [ ] 页面测试只验证渲染与 intent 委托，不复制 projection 或补偿规则。
- [ ] Project custom Skill 的 managed placements 继续不会被 scan 当作 canonical source，也不会泄漏到 Project Symlink Inventory。
- [ ] scan/reconcile 与任意 Skill mutation 共享写入锁；并发回归测试证明 intent 与 scan 不会互相删除或覆盖 Distribution 状态。
- [ ] 所有 Skill 数据 mutation 成功后使用权威完整 catalog 替换 cache；Project custom intent 还会正确失效 Project counts。
- [ ] transaction 内先完成完整 catalog 构建再 commit；catalog 构建失败或 commit 失败都会 rollback DB 并补偿文件系统，commit 后不再执行会改变成功结果的 fallible 读取。
- [ ] deletion test 复核表明删除深化后的 module 会让身份、target projection、写入计划与补偿复杂度重新扩散到多个调用者。

## Out of Scope

- 统一 Skill、Prompt、Session 的自定义源数据形态。
- 修改 ADR-0003。
- 修改 Agent-sourced Skill source relocation；后续由 `07-14-deepen-distribution-source-relocation` 独立处理。
- 重新设计 Agent Matrix 视觉、搜索、tab 或 Project chip。
- 为崩溃恢复增加 durable operation journal。
- 新增 Repair UI、Repair command 或任意 target path 输入。
