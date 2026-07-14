# 深化 Project custom Skill 传播模块 — 设计

## 1. Module 与 Seam

外部 seam 继续位于 `SkillService`。新增 crate-private `services/project_custom_skill_propagation.rs`，集中 Project custom Skill 的 read projection、target 状态、用户 intent、Placement 计划、补偿与 reconciliation；Tauri command 和 React Query 只是 adapter。

不把该 implementation 塞入现有 `distribution::write_target`：后者是 Skill / Prompt 单 Placement 的通用 deep module，本任务需要的是 Project custom Skill 特有的 Global / Project 多 Placement 编排。后续 source relocation 由独立子任务处理，避免同时重写 `distribution.rs`。

Deletion test：删除新 module 后，canonical/incoming identity、Project target join、默认 Agent、文件系统计划和补偿会重新扩散到 `skills.rs`、两个页面、query hooks 与 Tauri adapter，因此 module 有真实 depth。

## 2. Read Interface

`list_skills` / `scan_skills` 名称不变，返回显式 `Vec<SkillRow>`。以下为契约形状，字段最终遵循 serde camelCase：

```rust
#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SkillRow {
    AgentCanonical {
        row_key: String,
        skill: SkillSummary,
        context: SkillContext,
        source_agent: String,
        cells: BTreeMap<String, AgentCellRole>,
    },
    ProjectCustomCanonical {
        row_key: String,
        skill: SkillSummary,
        source_project: ProjectRef,
        destinations: Vec<ProjectCustomDestinationState>,
    },
    ProjectCustomIncoming {
        row_key: String,
        skill: SkillSummary,
        source_project: ProjectRef,
        target_project: ProjectRef,
        cells: BTreeMap<String, PlacementCellRole>,
    },
}

#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub skill_id: String, // canonical skills.id
    pub name: String,
    pub desc: String,
    pub path: String,     // canonical source display path
    pub disabled: bool,
}

#[serde(rename_all = "camelCase")]
pub enum AgentCellRole { Source, Target, None }

#[serde(rename_all = "camelCase")]
pub enum PlacementCellRole { Target, None }
```

### Read invariants

1. `row_key` 只用于 React identity，不能作为写入 identity。
2. 每个 variant 的 `skill.skill_id` 都是 canonical `skills.id`；Open / Reveal / DMI 直接使用它。
3. Agent canonical row 必须恰有一个 source cell。
4. Project custom canonical / incoming cells 的类型只允许 target / none。
5. Project custom incoming 必须同时拥有 source Project 与 target Project。
6. source Project 允许同时拥有 canonical row 和指向自身的 incoming row。
7. incoming 仅在 destination 至少有一个 live target 时存在。
8. destination 顺序固定为 Global 在前，其后按 Project display order 返回全部可传播 Projects；健康的 source Project 包含在内。
9. 每个 destination 只通过 `cells` 表达状态；`enabled` 与 `targetAgents` 是调用者可纯派生的展示事实，不进入 wire shape。
10. core 只返回领域 facts，不返回按钮文案、toast、CSS 或 path preview。

`ProjectCustomDestinationState` 使用显式 variant，并以唯一一份 Placement cells 表达状态：

```rust
#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomDestinationState {
    Global { cells: BTreeMap<String, PlacementCellRole> },
    Project {
        project: ProjectRef,
        cells: BTreeMap<String, PlacementCellRole>,
    },
}
```

`enabled` 与 `targetAgents` 均从 destination cells 派生，不再返回第二份 Global 状态。Rust JSON serialization contract test固定 `rowKey` / `skillId` / `sourceProject` 等 camelCase 字段，并与 TypeScript fixture 对照。

这是 eager read model。当前本地规模允许 `Project custom Skills × 可传播 Projects` 的 payload；不增加 modal-open lazy query。可传播 Project 必须数据库状态为 active，且当前 Project Path 可解析、存在；stale / hidden / missing path Project 不进入 destinations。有效状态判断抽为 `projects` module 的 crate-private helper，由 Project 列表和 Skill propagation 复用。

## 3. Write Interface

新增唯一 Project custom Skill 写入口：

```rust
pub fn apply_project_custom_skill_intent(
    &self,
    intent: ProjectCustomSkillIntent,
) -> AppResult<ProjectCustomSkillMutationResult>;

#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomSkillIntent {
    SetTargetEnabled {
        skill_id: String,
        destination: ProjectCustomSkillDestination,
        enabled: bool,
    },
    SetAgentPlacement {
        skill_id: String,
        destination: ProjectCustomSkillDestination,
        agent: String,
        enabled: bool,
    },
}

#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomSkillDestination {
    Global,
    Project { project_id: String },
}

pub struct ProjectCustomSkillMutationResult {
    pub changed: bool,
    pub skills: Vec<SkillRow>,
}
```

### Intent semantics

- `SetTargetEnabled(true)`：destination 已启用时幂等 no-op；否则后端解析 Settings 默认入口 Agent并创建首个 Placement。
- `SetTargetEnabled(false)`：一次删除该 destination 的全部 Agent placements；空 destination 幂等 no-op。
- `SetAgentPlacement`：精确设置一个 Agent cell，不使用 toggle 语义；删除 Project 的末位 placement 后 incoming row 消失。
- Global 与 Project 共用 intent，但路径、表和 natural key 差异留在私有 implementation。
- agent-sourced Skill、非可传播 Project、无 Skill surface Agent 与目标路径冲突均在 core 拒绝；数据库 active 但 Project Path 缺失的记录按 stale 处理，不能创建新目录。

旧 `set_project_skill_project`、`set_project_skill_target` 及 Project custom 的 `set_skill_target` 路径直接删除，不保留兼容 adapter。普通 agent-sourced `set_skill_target` 继续存在，并显式拒绝 Project custom Skill。

### Retained mutation returns

为避免新 read model 下出现单行更新与 projection metadata 分叉，所有保留的数据 mutation 都返回权威 `Vec<SkillRow>`：

- `set_skill_target(...) -> AppResult<Vec<SkillRow>>`
- `move_skill_source(...) -> AppResult<Vec<SkillRow>>`
- `set_skill_disabled(...) -> AppResult<Vec<SkillRow>>`
- `apply_project_custom_skill_intent(...) -> AppResult<ProjectCustomSkillMutationResult>`，其中 result 携带完整 catalog。

Open / Reveal 仍返回 `AppResult<()>`。React Query 对所有数据 mutation 统一整体替换 `skillKeys.all`；只有 Project custom intent 额外 invalidate `projectKeys.all`。

## 4. Default Entry Agent

`SkillService` 构造时接收共享的 `AppConfigService`，不在 command 或页面临时创建依赖：

```rust
SkillService::new(db.clone(), app_config.clone())
```

执行 `SetTargetEnabled(true)` 时：

1. 读取 `AgentDisplayPreferences.default_global_entry_agent`；
2. 再验证 canonical Agent、Skill capability 与 disabled 状态；
3. 未设置或已清除时回退 canonical-leftmost `Generic Agent`；
4. intent 不接受 `defaultAgent`，读取 snapshot 也不作为执行输入。

Eager read model 不返回默认入口 Agent，也不按 disabled preferences 过滤 destinations，因此 Agent preferences 变化无需 invalidate Skill query；每次首次 enable 都在执行时读取最新设置。

## 5. Compensating Atomicity

### Serialization and mutation lock

`SkillService` 持有共享私有 mutation lock。lock 覆盖所有会改写 Skills / Distribution 同一事实的完整操作：`scan_skills` 的发现、replace 与 reconcile 阶段，普通 target 写入、source relocation、DMI，以及 Project custom intent 的 plan→filesystem→transaction。`list_skills` 只读，可直接通过数据库 snapshot 构造 catalog。

这样 scan/reconcile 不会在 intent 文件步骤与 DB transaction 之间删除或重写 Distribution rows。并发测试至少覆盖 intent 与 scan/reconcile 竞争。

### Plan

每次 Project custom intent：

1. 获取 Skill mutation lock，规范化 input，解析默认 Agent（如需要）。
2. 读取 canonical Skill、有效 Project 状态、Agent Capability 与当前 Distribution rows。
3. 计算 current set、desired set 与确定性 `PlacementPlan`。
4. 在任何写入前预检全部路径：
   - create：缺失可创建；已正确指向 canonical source 则视为已完成；其他内容为 conflict；
   - remove：缺失视为已完成；只有指向当前 canonical source 的 managed Placement 才允许删除。
5. 按 canonical Agent order 执行文件系统步骤，每一步记录逆操作。
6. 全部文件步骤成功后开启单个 SQLite transaction，在 transaction 内替换该 destination 的 Distribution rows、resolve 对应 evidence，并构造完整 Skill catalog。
7. catalog 构造成功后 commit，并直接返回 commit 前已构造的 catalog；commit 后不再执行 fallible DB read。
8. catalog 构造失败、commit 失败或此前任一步失败时，transaction rollback，并按实际完成的文件步骤逆序补偿。

### Failure outcomes

- 补偿全部成功：Distribution 数据保持调用前状态，返回原始 Validation / IO / Database error。
- 补偿失败：返回 `AppError::Reconciliation`，并在失败发生后写入窄 evidence 表。
- 不允许 best-effort 部分成功作为正常结果。
- 不为进程崩溃预写 journal，也不在启动或 scan 时执行恢复。

### Reconciliation evidence

Schema v21 新增 `skill_propagation_reconciliations`，仅在补偿失败时插入：

```text
id TEXT PRIMARY KEY
skill_id TEXT NOT NULL
destination_kind TEXT NOT NULL
target_project_id TEXT
intent_json TEXT NOT NULL
completed_steps_json TEXT NOT NULL
failed_compensations_json TEXT NOT NULL
observed_paths_json TEXT NOT NULL
created_at INTEGER NOT NULL
resolved_at INTEGER
```

- evidence 不是 Distribution 真相源，不设置会随 Skill 删除而丢失的 foreign key。
- reconciliation error message 包含 evidence ID；若 evidence 持久化也失败，错误必须同时包含该失败，不得覆盖原始补偿错误。
- 同一 intent 再次成功后，将同 Skill + destination 的未解决 evidence 标记 `resolved_at`。
- 不新增 evidence 查询、Repair UI 或 Repair command。

同一 intent 可重试：已正确创建的 Placement 与已完成的删除都视为 no-op，继续处理剩余步骤并收敛数据库。

## 6. Dependency Strategy

### In-process

- canonical/incoming projection、target join、排序、intent → desired set、Agent capability 校验。
- 直接放在 module implementation 内，不增加 adapter。

### Local-substitutable

- SQLite：生产使用现有 `Database`，测试使用 `open_in_memory()`。
- 文件系统：生产使用现有 managed directory link 原语，测试使用 `TempDir`。
- 补偿阶段故障注入使用 crate-private internal seam；生产 OS executor 与 scripted test executor 是两个真实 adapter，但不暴露到 Tauri / TypeScript interface。
- 确定性的“正向第 N 步失败 / 补偿失败”矩阵放在新 module 的 `#[cfg(test)]` unit tests，通过私有 coordinator/executor interface 驱动；`crates/nexus-core/tests/skill_service.rs` 继续从公开 `SkillService` interface 验证真实 SQLite + TempDir 的可观察结果。

### Existing module reuse

- 复用 Agent Capability Surface、`distribution::placement_points_to`、managed directory link 与路径 helper。
- 不为 Database 抽 port，不创建通用 Asset propagation trait。

## 7. Frontend Data Flow

```text
SkillPage / ProjectDetailView / SkillRow
  -> useApplyProjectCustomSkillIntentMutation
  -> skillsApi.applyProjectCustomSkillIntent
  -> Tauri command adapter
  -> SkillService.apply_project_custom_skill_intent
  -> private Project custom propagation module
  -> filesystem Placement + SQLite
  -> authoritative Vec<SkillRow>
  -> replace skillKeys.all
  -> Project custom intent additionally invalidates projectKeys.all
```

- `src-react/src/types/index.ts` 用 discriminated union 对齐 serde。
- `SkillRow` 按 `kind` exhaustive render；保留独立行与现有 Agent Matrix。
- 多个传播 callback 收敛为一个窄 `onProjectCustomIntent`。
- `SkillPage` 与 `ProjectDetailView` 只构造 typed intent、展示 toast，不学习 query key、默认 Agent 或补偿顺序。
- 删除 `components/skill/propagation.ts`。
- `visibility.ts` 与页面 selector 改为按 row variant / context 选择，不再推断 optional fields。
- 所有数据 mutation 均消费权威完整 catalog并整体替换 Skill cache，避免 DMI、source relocation 或普通 target mutation 留下 projection metadata stale；不把 DMI、Open/Reveal 或 source relocation塞入宽 `useSkillSurface`。
- eager destinations 依赖 Project 集合、有效状态、名称与 display order。`useRecordProjectMutation`、批量 record、delete、reorder，以及任何会续认/移动 Project Path 的写入成功后都必须 invalidate `skillKeys.all`；对应 query tests 固定该关系。纯 Git Base Folder 列表变化或只返回候选的 scan 不触发无关 invalidation。

## 8. Tauri Adapter

`src-tauri/src/commands/skills.rs` 保持薄：

- `list_skills` / `scan_skills` 返回 `Vec<SkillRow>`。
- `set_skill_target`、`move_skill_source`、`set_skill_disabled` 返回 `Vec<SkillRow>`，与 core 保留 mutation contract 对齐。
- 新增 `apply_project_custom_skill_intent`，直接委托 core。
- 删除两个旧 Project custom commands。
- `src-tauri/src/lib.rs` 更新 handler 注册。
- `store.rs` 先创建并 clone `AppConfigService`，注入 `SkillService`。

`nexus-core` 不引入 Tauri 类型或 runtime。

## 9. Test Surface

### Core interface tests

通过 `SkillService` / 私有 deep module interface 覆盖：

- 三种 row variants、独立 role enums、serde camelCase JSON contract 与不可出现的 source cell。
- Global 唯一状态、健康 source Project 与跨 Project destination facts。
- stale / hidden / missing-path Project 排除与 intent 拒绝。
- Settings 默认 Agent 与 Generic Agent fallback。
- target enable/withdraw、Agent fan-out、末位删除。
- 重复 intent 幂等。
- 目标路径冲突不覆盖。
- 第 N 个文件步骤失败后的逆序补偿与 DB 不变。
- catalog 在 transaction 内构建失败、DB commit 失败后的 rollback 与文件补偿；commit 后无 fallible catalog read。
- 补偿失败的 reconciliation kind、evidence row 与同 intent 重试收敛。
- scan/reconcile 与 intent 并发时不删除或覆盖 Distribution 状态。
- fresh database 与 v20 → v21 migration 均创建 evidence 表并写入 schema version 21。
- rescan 不把 Placement 当 canonical source。
- Project Symlink Inventory 继续隐藏真实 managed Placement，但不隐藏被替换链接。

### Frontend tests

- discriminated union fixture 与 exhaustive selectors。
- SkillRow canonical/incoming 渲染与单 intent 委托。
- query mutation 权威全量替换 Skill cache并失效 Project query。
- 页面不循环 mutation，不传 `defaultAgent`，不解析 composite identity。

旧 shallow helper 测试不叠加保留；测试移动到 deep module interface。

## 10. Compatibility、Rollout 与 Rollback

- 项目未上线，不兼容旧 payload / command；schema v21 采用最小顺序 migration。
- 保持数据库中的 canonical Skill 与两张 Distribution 表语义不变，仅新增 reconciliation evidence 表。
- 实施顺序为 core read types → projection → intent/atomicity → Tauri adapter → TS types/API/query → 页面 → 删除旧 interface。
- 每一阶段保持编译通过；不要长期提交新旧写 interface 并存状态。
- 回滚时可删除 evidence migration 与新 module，恢复旧 DTO/commands；现有 canonical/distribution 数据无需转换。
- 本任务先于 `07-14-deepen-distribution-source-relocation` 实施，两者都修改 `skills.rs`，不得并行。
