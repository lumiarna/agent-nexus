# 技术设计：深化 Distribution source relocation

## 1. 设计目标

在现有 `services::distribution` seam 上建立一个 deep module：调用者只表达“把某个资产的 source 移到目标 Agent”，Distribution implementation 隐藏目标检查、文件系统顺序、数据库角色切换和逆序补偿。

设计采用 **trait adapter + module 内部声明式 plan + 私有 typed compensation journal**。这综合了 design-it-twice 的调用者优先方案与 operation 方案的故障诊断能力。

## 2. Module 与 interface

建议的 crate-private interface 形状：

```rust
pub(crate) fn relocate_source<A: SourceRelocationAdapter>(
    adapter: &A,
    asset_id: &str,
    target_agent: &'static AgentCapabilitySurface,
) -> AppResult<RelocationOutcome>;

pub(crate) trait SourceRelocationAdapter {
    type Metadata;

    fn database(&self) -> &Database;

    fn plan_relocation(
        &self,
        asset_id: &str,
        target_agent: &'static AgentCapabilitySurface,
    ) -> AppResult<RelocationPlan<Self::Metadata>>;

    fn persist_asset_move(
        &self,
        tx: &rusqlite::Transaction<'_>,
        movement: &PreparedSourceMove<Self::Metadata>,
        now: i64,
    ) -> AppResult<()>;
}
```

具体命名可在实现时按 Rust 借用约束微调，但 interface 必须保持以下性质：

- 面向调用者只有一个 relocation 入口。
- plan 阶段只读取、校验和计算，不产生 mutation。
- adapter 不暴露 remove、rename、place、rollback 等逐步回调。
- 资产持久化接收 Distribution 已开启的 transaction，不自行提交。
- 共同 Distribution role 更新不由 adapter 实现。

## 3. 内部 plan

`RelocationPlan` 是 module 内部 representation，不由 Skill/Prompt 调用者手工组装：

```rust
enum RelocationPlan<M> {
    Unchanged,
    Move(PreparedSourceMove<M>),
}

struct PreparedSourceMove<M> {
    asset_id: String,
    storage: DistributionStorage,
    old_source_agent: &'static str,
    new_source_agent: &'static str,
    old_canonical_path: PathBuf,
    new_canonical_path: PathBuf,
    placement_kind: PlacementKind,
    metadata: M,
}

enum DistributionStorage { Skill, Prompt }
enum PlacementKind { Directory, File }
```

`DistributionStorage` 在 `distribution.rs` 内映射固定表名和 ID 列，避免 adapter 或用户输入提供动态 SQL identifier。

目标状态不能只由数据库 `role = target` 推断。preflight 应区分：

- `Vacant`：目标不存在，或记录为 target 但 placement 已缺失；
- `ManagedPlacement`：目标确实是指向旧 canonical source 的受管 placement；
- `Conflict`：目标存在非托管内容或指向其他来源。

只有 `ManagedPlacement` 可在 relocation 中移除；`Conflict` 必须在破坏性 mutation 前失败。

## 4. Adapter 职责

### 4.1 Skill adapter

保留：

- 目标 Agent 的 Skill capability 校验；
- `source_kind = agent` 校验；
- global/project Skill 目标路径计算；
- directory placement 选择；
- transaction 内更新 `skills.canonical_path`、`skills.source_agent`、`updated_at`。

不再保留：目标 role 查询、placement 移除时序、rename、旧 source placement 创建、role upsert 和 rollback。

### 4.2 Prompt adapter

保留：

- 目标 Agent 的 Prompt capability/scope 校验；
- global Prompt path 与 project Prompt stem-swap；
- file placement 选择；
- display name 计算；
- Project Prompt old/new relative path 计算；
- transaction 内更新 `prompts.name`、`canonical_path`、`updated_at`；
- 在同一 transaction 中更新 Extra Prompt File 登记。

Prompt Extra Prompt File 仍属于 Agent prompt namespace，不建模为 Skill 式 Project Custom Source。

## 5. 执行顺序

Distribution implementation 固定执行：

1. adapter 生成 plan；同 source 返回 `Unchanged`，不产生 I/O 或 DB mutation。
2. 验证数据库中当前 source 与 plan 一致，并读取目标 role。
3. 检查旧 canonical source 与目标路径状态。
4. 目标为 managed target 时移除该 placement，并记录补偿步骤。
5. 创建必要的目标父目录。
6. rename 旧 canonical source 到新 canonical path，并记录补偿步骤。
7. 在旧 canonical path 创建指向新 canonical source 的 placement，并记录补偿步骤。
8. 开启 SQLite transaction。
9. adapter 更新资产特有字段和 Prompt extra 登记。
10. Distribution module 将旧 source Agent upsert 为 `target`，target path 为旧 canonical path。
11. Distribution module 将目标 Agent upsert 为唯一 `source`，target path 为 `NULL`。
12. commit；成功后清空 compensation journal。

数据库事务覆盖资产字段、Prompt extra 登记及 role 更新。文件系统不能加入 SQLite transaction，因此通过补偿协调。

## 6. 补偿与错误契约

使用 module-private typed journal，正向步骤成功后才记录对应 undo：

```rust
enum UndoStep {
    RestoreRemovedTarget { /* kind, source, target */ },
    RenameCanonicalBack { /* from, to */ },
    RemoveOldSourcePlacement { /* kind, source, target */ },
    RemoveCreatedParents { /* only operation-created empty dirs */ },
}
```

规则：

- 原始执行失败后，transaction 由 rusqlite rollback。
- journal 从尾到头执行；一个补偿失败后仍继续尝试更早步骤。
- 所有补偿成功时返回原始 `Validation` / `Io` / `Database` 错误。
- 任一补偿失败时返回 `AppError::Reconciliation`，内容至少包含原始失败阶段、原始错误、失败的补偿步骤及 old/new path。
- 不静默吞掉补偿错误。
- 不持久化 journal；进程崩溃恢复不在本任务范围。

## 7. 私有故障注入 seam

共同状态机需要稳定覆盖 rename、placement、数据库阶段和补偿失败。允许在 `distribution.rs` implementation 内建立 crate-private/test-private runtime seam：

```rust
trait RelocationRuntime {
    fn inspect_destination(...);
    fn remove_placement(...);
    fn create_parents(...);
    fn rename(...);
    fn create_placement(...);
}
```

- 生产 adapter 执行真实文件系统操作。
- scripted test adapter 记录顺序并在指定阶段失败。
- 此 seam 不进入 Skill/Prompt 调用 interface。
- SQLite 继续使用真实 in-memory `Database`，不抽 persistence adapter。

若实现中能用更小的私有 seam 达成同等 deterministic coverage，应优先更小形状。

## 8. 测试边界

### Distribution module interface

共同测试一次：

- no-op；
- vacant target 成功；
- managed target 成功；
- conflict 在 mutation 前失败；
- remove、rename、old-source placement、资产 DB 更新、role 更新失败；
- journal 严格逆序；
- 补偿失败继续执行剩余 undo，并返回 `Reconciliation`；
- 成功后唯一 source、旧 source 为 target、路径关系正确。

### Skill adapter

验证 Agent-sourced 限制、global/project directory path、目录 placement，以及返回 authoritative catalog。

### Prompt adapter

验证 global file path、project primary/extra stem-swap、display name、Extra Prompt File 更新、rescan 兼容和文件 placement。

旧测试若重复验证共同状态机，应由 Distribution interface 测试替换，而不是继续叠加。

## 9. 兼容性与回滚

- 不修改 Tauri IPC、前端调用或数据库 schema。
- `move_skill_source` 与 `move_prompt_source` 的返回形状保持不变。
- target toggle 继续复用现有 `distribution::write_target`。
- 若深化出现不可控回归，可回滚 relocation interface 与两个 adapter 改动；无 schema 数据需要回滚。

## 10. Trade-off 与 deletion test

该设计用更复杂的 Distribution implementation 换取更小的 interface，这是有意的 depth。删除深化后的 relocation interface，会迫使 Skill/Prompt 各自重新实现目标判断、执行顺序、role 更新、补偿、故障诊断与测试矩阵，因此复杂度会重新扩散，module 通过 deletion test。

拒绝：

- 多个 helper 让调用者手工编排：仍是 shallow pass-through。
- 暴露逐阶段 trait 方法：interface 复述 implementation。
- durable saga/journal：当前没有崩溃恢复需求。
- 预接入第三种资产或 persistence adapter：没有真实变化轴。
