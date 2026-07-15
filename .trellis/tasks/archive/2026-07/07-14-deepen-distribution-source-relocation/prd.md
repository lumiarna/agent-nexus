# 深化 Distribution source relocation

## Goal

把 Skill 与 Prompt 重复实现的 source relocation 状态机和失败补偿收回现有 Distribution module，使调用者只表达资产特有差异，并让共同顺序、不变量和补偿行为获得单一 locality。

**推荐强度：Strong**

## Background

- Skill relocation 当前位于 `crates/nexus-core/src/services/skills.rs:383-473`。
- Prompt relocation 当前位于 `crates/nexus-core/src/services/prompts.rs:278-375`。
- 两侧重复维护目标 role 查询、目标 placement 移除、路径冲突检查、canonical source rename、旧 source placement 创建、数据库角色更新与逆序补偿。
- 两侧数据库更新分别位于 `skills.rs` 和 `prompts.rs`；Prompt 还必须在同一事务中维护 Extra Prompt File 登记。
- `crates/nexus-core/src/services/distribution.rs` 已承载 Agent Matrix、target placement 写入和共同 Distribution 查询，是本次深化的既有真实 seam。
- `4e3f816` 同时大幅修改 Skill 与 Prompt relocation 路径，证明共同编排尚未被 Distribution module 吸收。
- SQLite 与文件系统均为 local-substitutable 依赖；Skill 和 Prompt 已构成两个真实 adapter，不需要为未来资产增加假想 adapter。

## Requirements

1. 深化现有 `distribution.rs`，不得在其外增加 pass-through module。
2. Distribution module 必须集中维护以下共同知识：
   - relocation preflight；
   - 目标 placement 状态识别；
   - remove → rename → place-old-source → persist 的顺序；
   - source/target role 更新；
   - 失败后的逆序补偿及补偿失败报告。
3. Skill adapter 仅保留 Agent-sourced 校验、Skill 路径计算、目录 placement 和 Skill 资产行更新。
4. Prompt adapter 仅保留 Prompt 路径计算、文件 placement、stem-swap、display name 与 Extra Prompt File 登记更新。
5. 保持 ADR-0003 对不同资产物理形态的决定，不把 Skill 与 Prompt 强行统一成相同数据形态。
6. 复用 Skill/Prompt 两个既有 adapter 形成的真实 seam，不增加无变化需求的新资产或持久化 adapter。
7. 采用一个面向调用者的 relocation 入口；声明式 plan、补偿 journal 与故障注入 seam 只能作为 module 内部 implementation，不得把执行顺序暴露给调用者。
8. 资产字段、Prompt Extra Prompt File 登记和 Distribution role 必须在同一 SQLite transaction 中提交。
9. 正常执行阶段失败时，已完成的文件系统步骤必须逆序补偿；补偿成功后返回原始错误类型。
10. 补偿本身失败时，允许返回现有 `AppError::Reconciliation`，同时报告原始失败、补偿失败阶段和相关路径；本任务不承诺进程崩溃或连续 I/O 故障后的自动恢复。
11. 不得继续以 `let _ = ...` 静默丢弃 relocation 补偿错误。
12. 共同状态机测试只写一次；Skill 与 Prompt 测试只保留各自真实差异和必要端到端成功路径。

## Acceptance Criteria

- [ ] Skill 与 Prompt 不再各自维护完整 relocation 状态机。
- [ ] 调用者只提供资产 ID、目标 Agent 和对应资产 adapter，不需要理解 relocation 顺序或补偿步骤。
- [ ] 目标已被非托管内容占用时不覆盖目标，canonical source、旧 source placement 与数据库角色保持不变。
- [ ] 目标原为 managed target 时，成功 relocation 后目标成为唯一 source，旧 source 成为指向新 canonical source 的 target。
- [ ] rename 失败、旧 source placement 创建失败、资产数据库更新失败、Distribution role 更新失败均有共同测试验证逆序补偿。
- [ ] 补偿成功的失败路径恢复到操作前可观测状态；补偿失败返回包含原始错误和补偿上下文的 `Reconciliation` 错误。
- [ ] Skill global/project relocation 保持目录 placement 和现有路径规则。
- [ ] Prompt global/project relocation 保持文件 placement、stem-swap、display name 及 Project Prompt Extra Prompt File 登记行为。
- [ ] 共同状态机与故障矩阵在 Distribution module interface 上测试一次；两个 adapter 分别测试真实差异。
- [ ] 既有 Skill/Prompt 成功路径、target toggle 与 rescan 行为不回归。
- [ ] deletion test 复核表明：删除深化后的 relocation interface 会让顺序、role 更新、补偿和故障测试重新扩散到 Skill 与 Prompt。

## Out of Scope

- 修改 Agent Matrix 产品语义或前端交互。
- 合并 Skill 与 Prompt 的路径规则或自定义源数据形态。
- 引入新的持久化 adapter、数据库 schema 或独立 pass-through module。
- 引入 durable compensation journal、后台 reconciliation runner 或进程启动恢复。
- 将 Session 或其他未来资产预先接入 relocation interface。
- 为跨卷 rename 增加 copy fallback。
