# 深化 Distribution source relocation

## Goal

把 Skill 与 Prompt 重复实现的 source relocation 状态机和失败补偿收回现有 Distribution module，深化已有真实 seam，并让两个资产 adapter 只保留物理差异。

**推荐强度：Strong**

## Evidence

- Skill relocation：`crates/nexus-core/src/services/skills.rs:440-530`。
- Prompt relocation：`crates/nexus-core/src/services/prompts.rs:278-375`。
- 两侧重复 target role 查询和 source/target 数据更新：`skills.rs:976-1042`、`prompts.rs:622-691`。
- 两侧都要求调用者理解移除旧 placement、检查目标、rename、建立旧 source placement、更新数据库及逆序回滚。
- `4e3f816` 同时大幅修改 Skill 与 Prompt 路径，是共同编排未被 Distribution module 吸收的直接证据。

## Requirements

1. 深化现有 `distribution.rs`，不得在其外再增加 pass-through module。
2. 共同 relocation 顺序约束和失败补偿集中到 Distribution locality。
3. Skill adapter 保留目录 placement；Prompt adapter 保留文件 placement、Extra Prompt File 登记与 stem-swap 等真实差异。
4. 保持 ADR-0003 对不同资产物理形态的决定，不追求虚假统一。
5. 复用已有 Skill/Prompt 两个 adapter 形成的真实 seam，不增加无变化需求的新 adapter。
6. 实现前通过 design-it-twice 比较 interface 形状，本任务当前不指定最终 interface。

## Acceptance Criteria

- [ ] Skill 与 Prompt 不再各自维护完整 relocation 状态机。
- [ ] 目标已占用、目标原为 target、rename 失败、旧 source placement 创建失败、数据库更新失败均有可验证补偿行为。
- [ ] 任一失败阶段后 canonical source、旧 source target 与数据库角色保持一致或回到原状态。
- [ ] Skill 与 Prompt 的既有成功路径及 Project Prompt extra 登记行为保持兼容。
- [ ] 共同状态机测试只写一次；两个 adapter 各自测试真实差异。
- [ ] deletion test 复核表明删除深化后的 Distribution module 会让 relocation 复杂度重新扩散到两个资产 adapter。

## Out of Scope

- 修改 Agent Matrix 领域语义。
- 合并 Skill 与 Prompt 的路径规则。
- 引入新的持久化 adapter。
