# 实施计划：深化 Distribution source relocation

## 0. 开始前门禁

- [x] 用户审阅并批准 `prd.md`、`design.md` 与本计划。
- [x] 完成 `implement.jsonl` / `check.jsonl` 的真实 spec/research context 配置。
- [x] 运行 `task.py validate`，确认 planning artifacts 与 context 合法。
- [x] 仅在用户明确批准实现后运行 `task.py start`。

## 1. 建立 Distribution relocation interface

- [ ] 在 `crates/nexus-core/src/services/distribution.rs` 定义单一 relocation 入口、两个真实 asset adapter 的 trait 契约及内部 plan。
- [ ] 使用固定 `DistributionStorage` 映射表名/ID 列，不接受用户提供的 SQL identifier。
- [ ] 实现 no-op、source 一致性校验与目标 `Vacant / ManagedPlacement / Conflict` preflight。
- [ ] 保持 `write_target` 与现有 Matrix 功能不回归。

**Review gate：** interface 不暴露 remove/rename/rollback 顺序；Skill/Prompt 调用者无需手工组装完整 plan。

## 2. 集中共同状态机与补偿

- [ ] 实现 remove managed target → rename canonical → create old-source placement → transaction persist 的固定顺序。
- [ ] 将旧 source/new source role upsert 收入 Distribution module，并与资产更新共享 transaction。
- [ ] 实现 typed compensation journal 与逆序执行。
- [ ] 补偿成功时保留原始错误类型；补偿失败时返回现有 `AppError::Reconciliation`。
- [ ] 移除 relocation 路径中的静默 `let _ = ...` 补偿丢弃。
- [ ] 如需 deterministic failure coverage，增加最小的 module-private runtime seam；不得扩大生产调用 interface。

**Rollback point：** 此阶段完成后先只运行 Distribution module 测试；若 interface 迫使 adapter 暴露状态机步骤，返回第 1 步调整设计。

## 3. 迁移 Skill adapter

- [ ] 将 `SkillService::move_skill_source` 精简为输入校验、调用 relocation interface 和读取 authoritative catalog。
- [ ] Skill adapter 保留 Agent-sourced 校验、global/project path 计算、directory placement 与 Skill 资产字段更新。
- [ ] 删除 `skills.rs` 中重复的 role 查询、文件系统编排、role transaction 和 rollback 代码。
- [ ] 保持 mutation lock 与现有返回类型。

## 4. 迁移 Prompt adapter

- [ ] 将 `PromptService::move_prompt_source` 精简为输入校验、调用 relocation interface 和读取 Prompt。
- [ ] Prompt adapter 保留 file placement、global/project path、stem-swap、display name 和 Extra Prompt File metadata。
- [ ] 将 Prompt 资产字段与 Extra Prompt File 更新放入 Distribution 提供的 transaction。
- [ ] 删除 `prompts.rs` 中重复的 role 查询、文件系统编排、role transaction 和 rollback 代码。

**Review gate：** ADR-0003 的 Skill/Prompt 物理差异仍位于各自 adapter；Distribution implementation 不出现 Prompt stem 或 Project Custom Skill 语义。

## 5. 替换测试而非叠加

- [ ] 在 Distribution module interface 上建立共同成功与故障矩阵：conflict、remove、rename、place-old-source、资产 DB、role DB、补偿失败。
- [ ] 断言可观测结果与顺序：canonical path、old-source placement、唯一 source、旧 source target、数据库回滚。
- [ ] Skill 测试只保留 Agent-sourced、global/project directory path 与端到端成功差异。
- [ ] Prompt 测试只保留 global/project file path、primary/extra stem-swap、Extra Prompt File、rescan 与端到端成功差异。
- [ ] 删除被新 Distribution interface 测试完全替代的重复状态机测试。

## 6. 验证

按项目约定执行：

```bash
pnpm rust:fmt
pnpm rust:check
pnpm rust:test
```

若只需先迭代单个 Rust 文件，可使用：

```bash
cargo fmt --all -- crates/nexus-core/src/services/distribution.rs
```

Windows 不直接运行裸 `cargo test -p nexus-core`；使用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`。

## 7. 最终检查

- [ ] 对照 PRD 逐项核验 acceptance criteria。
- [ ] 执行 deletion test：描述删除 relocation interface 后会扩散回 Skill/Prompt 的共同知识。
- [ ] 运行 Trellis quality check，检查 spec compliance、测试边界、重复逻辑与跨层回归。
- [ ] 确认无 schema、IPC 或前端非预期变化。
- [ ] 更新必要 spec 后再提交；提交与归档遵循 Phase 3。
