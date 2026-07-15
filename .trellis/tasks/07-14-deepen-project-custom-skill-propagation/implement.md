# 深化 Project custom Skill 传播模块 — 执行计划

## Preconditions

- 只实施本子任务，不同时启动 `07-14-deepen-distribution-source-relocation`。
- 先读取 `prd.md`、`design.md`、`research/design-it-twice.md` 与 manifests 中的 specs。
- 不安装依赖；复用现有 Rust、React Query、Vitest 与 tempfile 能力。
- Windows Rust 测试必须通过 `scripts/with-sqlite.mjs` / `pnpm rust:test`。

## Phase A — Baseline 与 RED Contract

- [x] 运行现有 Skill / Project Symlink Inventory、前端 unit/component 基线测试，记录非本任务失败。
- [x] 在 core tests 中先定义新的 `SkillRow` 三 variant、独立 cell role、camelCase JSON serialization、eager destinations 与 canonical `skillId` 契约。
- [x] 增加 intent 行为 RED tests：Global、健康 source Project、跨 Project、fan-out、末位删除、全量撤销与幂等。
- [x] 增加 stale / hidden / missing-path Project 排除与 intent 拒绝 RED tests。
- [x] 增加失败矩阵 RED tests：路径冲突、文件步骤失败、catalog 构建失败、DB commit 失败、补偿失败与重试收敛。
- [x] 增加 intent 与 scan/reconcile 并发 RED test。
- [x] 增加 fresh database 与 schema v20 → v21 reconciliation evidence migration RED tests。

### Review Gate A

- 确认测试只断言 module interface 的可观察结果，不锁定 SQL helper 或内部 plan 结构。
- 确认 Project custom canonical/incoming cells 不允许 source，source Project 自传播仍合法。

## Phase B — Core Read Model

- [x] 新增 crate-private `services/project_custom_skill_propagation.rs` 并在 `services/mod.rs` 注册为私有 implementation。
- [x] 在 `skills.rs` 定义公开 serde `SkillRow`、`SkillSummary`、Project refs、matrix cell 与 propagation target 类型。
- [x] 将 canonical row 与 incoming projection 构造迁入新 module；使用同一数据库 snapshot 生成 eager destination states，Global 只通过一份 Placement cells 表示。
- [x] 从 `projects` module 抽取并复用有效 Project 状态判断；数据库 active 但 Project Path 缺失时按 stale 排除，禁止传播创建目录。
- [x] 让 `list_skills` / `scan_skills` 返回 `Vec<SkillRow>`，保持 command 名称不变。
- [x] 保留 Agent canonical、Project custom canonical、Project custom incoming 的现有排序与可见语义。
- [x] 更新 Open / Reveal / DMI 等公共动作统一接受 canonical `skillId`。

### Validation B

```bash
node scripts/with-sqlite.mjs cargo test -p nexus-core --test skill_service
```

## Phase C — Intent 与补偿式原子性

- [x] 定义 `ProjectCustomSkillDestination`、`ProjectCustomSkillIntent` 与 mutation result。
- [x] 为 `SkillService` 增加共享私有 mutation lock，并覆盖 scan/reconcile、普通 target、source relocation、DMI 与 Project custom intent 的完整写流程。
- [x] 调整 `SkillService` 构造，显式接收共享 `AppConfigService`；后端解析并验证默认入口 Agent。
- [x] 实现 current → desired Placement plan 与全量 preflight。
- [x] create 遇到已正确 Placement、remove 遇到缺失 Placement 时按幂等已完成处理；错误内容不得被覆盖。
- [x] 按 canonical Agent order 执行文件步骤并记录逆操作；所有文件步骤成功后才开启 Distribution transaction。
- [x] 在 transaction 内完成 rows 更新、evidence resolution 与完整 catalog 构建，再 commit并返回已构建 catalog；commit 后不执行 fallible DB read。
- [x] catalog 构建、commit、文件或其他步骤失败时逆序补偿；普通失败不得留下 Distribution 部分提交。
- [x] 新增 `AppError::Reconciliation`，保持 Tauri serde `kind/message` 结构。
- [x] schema v21 新增 `skill_propagation_reconciliations`；只在补偿实际失败时写 evidence。
- [x] 同 intent 成功后标记相关 evidence resolved；不新增 Repair 或 crash recovery interface。
- [x] 在新 module 内的 `#[cfg(test)]` tests 使用 private scripted executor 覆盖第 N 步失败和补偿失败；外部 `skill_service.rs` 继续测试公开 interface，不暴露 filesystem port。

### Review Gate C

- 确认 reconciliation table 不是 Distribution 真相源、无自动恢复职责。
- 确认数据库写入发生在文件系统步骤全部成功之后。
- 确认重复 intent 能从实际 Placement 状态收敛，而非依赖上次进程内 plan。

## Phase D — Tauri Adapter

- [x] `commands/skills.rs` 的 list/scan 返回新 read model；保留的 `set_skill_target`、`move_skill_source`、`set_skill_disabled` 也统一返回完整 catalog。
- [x] 新增薄 `apply_project_custom_skill_intent` command。
- [x] 删除 `set_project_skill_project`、`set_project_skill_target` command 与 import。
- [x] `set_skill_target` 只处理 Agent-sourced Skill，并在 core 拒绝 Project custom Skill。
- [x] 更新 `lib.rs` command 注册。
- [x] `store.rs` 将共享 `AppConfigService` clone 注入 `SkillService`，不在 command 临时构造依赖。

## Phase E — TypeScript Contract 与 Query Adapter

- [x] 在 `types/index.ts` 用 discriminated union 替换 optional-heavy `Skill`。
- [x] 区分可含 source 的 Agent Matrix cells 与只含 target/none 的 Placement cells。
- [x] 更新 `lib/api/skills.ts`：添加统一 intent；删除两个旧 payload 和方法。
- [x] 更新 `lib/query/skills.ts`：所有 Skill 数据 mutation 成功后整体替换 `skillKeys.all`；Project custom intent 额外失效 Project query。
- [x] 为 record Project、批量 record、delete、reorder 及续认/移动 Project Path 的写入补齐 `skillKeys.all` invalidation；纯 Git Base Folder 列表变化或只返回候选的 scan 不增加无关 invalidation。
- [x] eager facts 不包含默认入口 Agent且不按 disabled preferences 过滤，因此不为 Agent preferences 增加 Skill invalidation。

### Validation E

```bash
pnpm typecheck
```

## Phase F — 页面与领域 UI

- [x] `SkillRow` 按 row variant exhaustive render，保持 canonical/incoming 独立行与现有 Agent Matrix。
- [x] 多个 Project custom propagation callbacks 收敛为单一 `onProjectCustomIntent`。
- [x] Propagate modal 直接消费 core target facts；path preview 改为不泄漏具体默认 Agent路径的呈现事实。
- [x] `SkillPage` 与 `ProjectDetailView` 删除默认 Agent读取、target join、Global 撤销循环、projection identity fallback 与重复编排。
- [x] 更新 `visibility.ts` 和 Project/Scope selectors，按显式 variant/context 选择 rows。
- [x] 删除 `components/skill/propagation.ts` 及所有旧 helper/import。
- [x] 保持搜索、Project chip、DMI、Open/Reveal、source tooltip 与 toast 用户行为。

## Phase G — Tests 与旧 Interface 清理

- [x] 迁移 `skill_service.rs` 既有传播测试到新 read/write interface，保留所有原行为断言。
- [x] 增加 Project Symlink Inventory 回归：真实 managed Placement 隐藏，被替换链接仍显示。
- [x] 增加 frontend discriminated union JSON fixture、selector / visibility unit tests，固定 camelCase contract。
- [x] 增加 SkillRow component tests：source target Add/Remove 只发送一个 intent，incoming cell 发送 Agent placement intent。
- [x] 增加 query component tests：完整 catalog 替换、Project query invalidation、失败不污染 cache。
- [x] 删除只验证旧 optional DTO、`computePropagationTargets` 或旧 command payload 的测试。
- [x] 全仓搜索并移除 `canonicalSkillId`、`placementScope`、`placementProjectId`、`SetProjectSkillProjectInput`、`SetProjectSkillTargetInput`。

## Full Validation

```bash
# Rust targeted
node scripts/with-sqlite.mjs cargo test -p nexus-core --test skill_service
node scripts/with-sqlite.mjs cargo test -p nexus-core --test project_symlink_inventory
node scripts/with-sqlite.mjs cargo test -p nexus-core database::schema::tests

# Frontend
pnpm typecheck
pnpm --dir src-react test:unit
pnpm --dir src-react test:component
pnpm build

# Full Rust quality
pnpm rust:fmt
pnpm rust:lint
pnpm rust:test

# Repository hygiene
git diff --check
```

## Spec / Documentation Follow-up

- [x] 更新 `.trellis/spec/nexus-core/backend/database-guidelines.md` 的 Cross-project Project custom Skill interface、补偿与测试契约，删除旧 DTO/command 说明及错误的 `target!=source` 示例。
- [x] 更新 `.trellis/spec/nexus-core/backend/error-handling.md`，记录 `Reconciliation` kind、evidence 与同 intent 重试契约。
- [x] 更新 `.trellis/spec/agent-nexus/frontend/type-safety.md`，用 discriminated union 与 typed intent 替代 optional projection contract。
- [x] 更新 `docs/design/Database Schema.md` 的 schema v21 reconciliation evidence 表；如 Architecture Design 的 Skill/Distribution 描述与最终 seam 不一致，做最小同步。
- [x] 不修改 ADR-0003。

## Rollback Points

1. **Read model rollback**：在删除旧 DTO 前，若 union 无法覆盖现有页面，回退 `SkillRow` 迁移，不保留双 wire shape。
2. **Atomicity rollback**：若补偿计划无法稳定跨平台，保留新 read model，退回规划修正；不得降级为 best-effort 正常结果。
3. **Frontend rollback**：core 与 Tauri contract 合并后，前端必须同提交迁移；不提交新旧 commands 长期并存状态。
4. **Schema rollback**：项目未上线，可回退 v21 migration；现有 Skill/Distribution 数据无需迁移或恢复。

## Verification Record

- 任务范围内的 `skill_service`、`project_symlink_inventory`、schema、前端 unit/targeted component、typecheck、build、Rust fmt/check 均通过。
- 完整 component suite 的 `connectionForms.test.tsx` 3 个失败来自未改动的既有测试基线。
- 标准 `pnpm rust:lint` 仍被未改动的 `provider_trigger.rs` / TrayMetric 既有 Clippy 告警阻断；本任务未新增 lint。
- 完整 Rust suite 的 `codex_models_are_supported_when_auth_exists` 受既有网络环境影响；本任务 targeted Rust suites 通过。
