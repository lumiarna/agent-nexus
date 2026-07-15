# nexus-core Database Guidelines

## 适用范围

适用于 `crates/nexus-core/src/database/` 与 service 中的 rusqlite 访问。

## 基础模式

- 使用 rusqlite 直写 SQL，不使用 ORM。依据：`docs/design/Architecture Design.md`、`docs/design/Database Schema.md`。
- `Database` 持有 `Mutex<Connection>`，入口为 `open` / `open_in_memory` / `connection`。参考 `database/mod.rs`。
- 初始化时启用 `foreign_keys` 并运行 `schema::migrate`。
- 测试优先使用 `Database::open_in_memory()`，参考 `tests/sync_service.rs`、`tests/project_service.rs`。

## Migration 规则

- 当前 schema 版本由 `database/schema.rs` 的 `CURRENT_SCHEMA_VERSION` 管理。
- 新迁移按顺序新增 `migrate_to_vN`，并在 `migrate` 中串联；不要修改历史迁移含义来“修复”已发布版本。
- 项目尚未上线时，`GOTCHAS.md` 允许数据库迁移以最小成本实现，但仍要保持启动迁移路径可运行。
- Windows SQLite 动态链接依据 ADR-0001；测试命令不要绕过 `with-sqlite` 包装。

## 事务与不变量

- 涉及多表写入或“先文件系统 placement 后 DB”的操作要有回滚思路。参考 `services/distribution.rs::write_target` 和 `services/sync/task_lifecycle.rs` 创建 link placement 失败回滚。
- `Agent Matrix` 每个 agent 集合完整、source 唯一等不变量由 service 维护，不能只依赖 UI 或 partial unique index。
- `Sync Task` 的 `direction` 由 `source_type` / `target_type` 派生；`Cloud→Cloud` 非法；`Symlink` / `Junction` 仅限 Distribution。

### Convention: task_groups 系统记录防御（system_kind）

**What**: `task_groups.system_kind` 区分用户组（`NULL`）与系统内置组（`NOT NULL`，如 `session_backup`，固定 id `system:session-backup`）。所有面向用户组的 **查询/写入/删除** SQL，WHERE 必须带 `system_kind IS NULL`。

**Why**: 系统 group 由 reconciler 维护（`session_backup_reconciler.rs`），不应被用户 CRUD 命令误改/误删。漏掉该条件会破坏系统内置行为。`list_task_groups` / `reorder_task_groups` / `rename_task_group` 均带此条件；`delete_task_group` 当前**缺失**（已知不一致，待补）。

**Example**:
```rust
// Correct: WHERE 同时防「不存在」与「命中系统组」，rows_affected==0 一并判定
let affected = conn.execute(
    "UPDATE task_groups SET name = ?2, updated_at = ?3
     WHERE id = ?1 AND system_kind IS NULL",
    params![group_id, name, now],
)?;
if affected == 0 {
    return Err(AppError::Validation("task group not found".to_string()));
}

// Wrong: 漏 system_kind IS NULL → 可能误改系统组
conn.execute("UPDATE task_groups SET name = ?2 WHERE id = ?1", params![group_id, name])?;
```

**Related**: 用户组与系统组在 UI 层也分离——系统组走独立 query（`list_session_backups`）与独立 "System-managed records" section，不进入 `TaskGroupCard`。新增针对 `task_groups` 的写命令时，单测须含「系统组 id 报 `task group not found`」守门用例（参考 `tests/sync_service.rs::rename_task_group_rejects_system_group_id`）。

## 表和字段命名

- 表名和字段名对齐 `CONTEXT.md` 领域词：`projects`、`skills`、`prompts`、`providers`、`session_index`、`task_groups`、`tasks`、`skill_distributions`、`skill_project_distributions`（跨 Project 投影，见下文 Scenario）。
- 时间戳使用 Unix epoch seconds；主键多数为 UUID TEXT。
- JSON/text list 字段只用于弱结构或简单配置，例如 `connection_params`、换行分隔的 `custom_skills_dirs` / `extra_prompt_files`。

## Scenario: Project custom Skill 传播 deep module

### 1. Scope / Trigger
- Trigger：修改 `source_kind = project_custom` Skill 的读取投影、Global / Project 传播、`skill_project_distributions`、补偿或 scan reconcile。
- 只适用于 Project custom Skill；不得把 Prompt extras 或 Session Directory 抽入共同 seam（ADR-0003）。

### 2. Signatures
- Read：`list_skills()` / `scan_skills() -> AppResult<Vec<SkillRow>>`；`SkillRow` 是 `agentCanonical | projectCustomCanonical | projectCustomIncoming` serde enum。
- Write：`apply_project_custom_skill_intent(ProjectCustomSkillIntent) -> AppResult<ProjectCustomSkillMutationResult>`。
- Intent 仅有 `setTargetEnabled` 与 `setAgentPlacement`；destination 仅有 typed `global | project { projectId }`，不接受 `defaultAgent` 或 target path。
- Schema v21 新增 `skill_propagation_reconciliations(id, skill_id, destination_kind, target_project_id, intent_json, completed_steps_json, failed_compensations_json, observed_paths_json, created_at, resolved_at)`。
- 旧 `set_project_skill_project` / `set_project_skill_target` 已删除；普通 `set_skill_target` 明确拒绝 Project custom Skill。

### 3. Contracts
- 每个 row 都有只用于渲染的 `rowKey` 和 canonical `skill.skillId`；调用者不再解释 composite id。
- Agent canonical cells 使用 `AgentCellRole`（可含且恰有一个 `source`）；Project custom canonical / incoming cells 使用 `PlacementCellRole`（只能 `target | none`）。
- canonical row eager 返回唯一 Global state 与全部有效 Project destination；有效 Project = DB `active` 且当前 Project Path 可解析、存在。健康 source Project 也必须包含。
- incoming 仅在 destination 至少一个 live target 时存在；source Project 可同时有 canonical row 与指向自身的 incoming row。
- `SetTargetEnabled(true)` 在 destination 为空时由后端读取最新 Settings 默认入口 Agent；未设置回退 `Generic Agent`，并重新校验 Skill capability 与 disabled 状态。
- 所有领域与路径 preflight 在写入前完成；文件步骤全部成功后才在单个 transaction 替换 Distribution rows、resolve evidence、构建完整 catalog并 commit。commit 后不再执行 fallible catalog read。
- 任一步失败时逆序补偿；补偿成功保留原错误且 DB 不变，补偿失败返回 `Reconciliation` 并写 evidence。evidence 无 FK、不是 Distribution 真相源，也不承担启动恢复。
- 同一个私有 Skill mutation lock 覆盖 Project custom intent、scan/reconcile、普通 target、source relocation 与 DMI。
- Project destination 使用目标 Agent fixed project skills dir，绝不用目标 Project `custom_skills_dirs`；managed Placement 必须实际指向 canonical source 才可删除或从 Inventory 隐藏。

### 4. Validation & Error Matrix
- Skill 不存在 → `Validation("skill was not found")`。
- `source_kind != project_custom` → `Validation("only Project custom Skills accept propagation intents")`。
- canonical Skill 无 source Project → `Validation("skill has no source Project")`。
- target Project 不存在、hidden、stale 或 path 缺失 → `Validation("target project was not found or is not effectively active")`，且不得创建目录。
- Agent 未知或无 Skill surface → `Validation("unknown agent: ...")` / `Validation("<Agent> does not support skill placement")`。
- target path 被非托管内容占用或 managed Placement 已被替换 → Validation，原内容不得覆盖或删除。
- 正向步骤失败且补偿成功 → 返回原始 Validation / IO / Database；补偿也失败 → `Reconciliation`，message 包含 evidence ID。

### 5. Good/Base/Bad Cases
- Good：一次 `setTargetEnabled(false)` 删除 destination 全部 placements；DB 只在文件步骤全成功后提交，返回完整 catalog。
- Base：相同 intent 重试时，正确既有 Placement或已缺失 Placement视为已完成并继续收敛。
- Bad：页面逐 Agent 循环 mutation，或先提交 Distribution rows 再执行文件步骤，都会暴露部分成功状态。

### 6. Tests Required
- serde camelCase、三 row variants、两种 cell role 与 Project custom 无 source。
- Global、source Project、跨 Project、fan-out、末位删除、全量撤销、默认 Agent、幂等重试与路径冲突。
- stale / hidden / missing path 排除与拒绝；rescan 不把 managed Placement 当 canonical source。
- 文件步骤失败逆序补偿、补偿失败 evidence / Reconciliation、catalog/commit rollback，以及 intent 与 scan 共享锁的并发回归。
- fresh DB 与 v20→v21 migration 均创建 evidence 表；Project Symlink Inventory 保持 managed identity 校验。

### 7. Wrong vs Correct
#### Wrong
```rust
// 先写 DB，再逐个创建 Placement：中途失败会留下部分提交。
replace_distribution_rows(&conn, &desired)?;
for placement in desired { create_link(placement)?; }
```

#### Correct
```rust
let plan = preflight_all_paths(current, desired)?;
let completed = execute_files_with_undo(&plan)?;
let tx = conn.transaction()?;
replace_distribution_rows(&tx, &plan.desired)?;
let catalog = catalog_from_connection(&tx)?;
tx.commit()?;
return Ok(catalog);
```

## Scenario: Agent Matrix source move for Skill / Prompt

### 1. Scope / Trigger
- Trigger: 为 `Skill` 或 `Prompt` 的 Agent Matrix 增加/维护“移动 Source Agent”能力，涉及 service command、`*_distributions` source/target 角色、canonical 文件/目录移动和 managed placement 回滚。

### 2. Signatures
- Core service:
  - `SkillService::move_skill_source(MoveSkillSourceInput { skill_id, agent }) -> AppResult<Skill>`
  - `PromptService::move_prompt_source(MovePromptSourceInput { prompt_id, agent }) -> AppResult<Prompt>`
- Tauri command:
  - `move_skill_source(input: MoveSkillSourceInput) -> AppResult<Skill>`
  - `move_prompt_source(input: MovePromptSourceInput) -> AppResult<Prompt>`
- Frontend IPC payload uses camelCase: `{ skillId, agent }` / `{ promptId, agent }`.

### 3. Contracts
- 只允许 agent-sourced canonical asset 移动 source；`project_custom` Skill 没有 Agent source，不能通过该命令赋予 source。
- 移动后必须恰好一个 `source` row：目标 Agent 变 `source`，旧 source Agent 变 `target`，旧 canonical path 成为指向新 canonical path 的 managed placement。
- 若目标 Agent 原本是 `target`，先移除目标 managed placement，再把 canonical source 移动到目标 path；不能覆盖非托管文件/目录。
- 文件系统操作早于 DB 写入时必须有回滚思路：DB 更新失败要尽力移回 canonical path、移除新建旧 source placement，并恢复原目标 target placement。
- Prompt project extra file 从 `AGENTS*.md` / `CLAUDE*.md` namespace 互换时，必须同步更新 `projects.extra_prompt_files`，否则 rescan 后 canonical row 会丢失。

### 4. Validation & Error Matrix
- asset 不存在 -> `Validation("skill was not found")` / `Validation("prompt was not found")`
- Skill `source_kind != 'agent'` -> `Validation("only Agent-sourced Skills can move source")`
- 目标 Agent 无对应 surface -> validation error (`does not support skill placement` / `does not support prompt targets`)
- 目标 Agent 已是当前 source -> 返回当前 asset，不做破坏性操作
- 目标 path 存在非托管内容或真实 IO 错误 -> 返回错误，不覆盖，尽力恢复已移除 target placement

### 5. Good/Base/Bad Cases
- Good: Ctrl-click `target` cell 后目标 Agent 唯一 `source`，旧 source 是 `target`，旧 canonical path 是 managed link。
- Base: 普通点击仍走 `set_skill_target` / `set_prompt_target`，只切换 target/none，不移动 canonical path。
- Bad: 只更新 DB 的 `source_agent` 而不移动 canonical 文件/目录；下一次 scan 会按真实文件位置把 source 又识别回旧 Agent。

### 6. Tests Required
- `crates/nexus-core/tests/skill_service.rs`: agent-sourced Skill move source，assert source 唯一、旧 source 变 target、旧 source path link 指向新 canonical path。
- `crates/nexus-core/tests/skill_service.rs`: Project custom Skill 调 move source 被拒绝。
- `crates/nexus-core/tests/prompt_service.rs`: global/project Prompt move source，assert source 唯一、旧 source target link、内容仍可读。
- Extra prompt case：移动 project extra prompt 后 assert `projects.extra_prompt_files` 从旧相对路径更新到新相对路径，并且 rescan 后 row 保留。

### 7. Wrong vs Correct
#### Wrong
```rust
// 只改 DB，不移动 canonical source；scan 会按文件系统事实覆盖回来。
UPDATE skills SET source_agent = 'Claude Code' WHERE id = ?1;
```

#### Correct
```rust
// 先移除目标 managed target，移动 canonical path，创建旧 source placement，
// 再在事务中更新 canonical_path/source_agent 与 distribution rows；失败则尽力回滚文件系统。
service.move_skill_source(MoveSkillSourceInput { skill_id, agent })?;
```

## 常见错误 / anti-pattern

- 裸写 SQL 时忘记 `ON DELETE CASCADE` 或 service 级完整性检查。
- 将 Session 正文写入数据库；设计文档要求 session_index 只存元数据和摘要，正文留文件系统/Cloud。
- 把 `Project Path` 当稳定身份；稳定身份是 `Project Key`。
- 把跨 Project placement 落到目标 `customSkillsDirs` 而非默认 Agent fixed project skills dir——会被 rescan 误判为 canonical source。

## Scenario: Project Symlink Inventory managed identity

### 1. Scope / Trigger
- Trigger: 修改 `ProjectSymlinkInventory`、`project_managed_target_identities` 或新增会创建项目目录 symlink/junction 的 Distribution placement。

### 2. Signatures
- `distribution::project_managed_target_identities(&Connection) -> AppResult<HashSet<String>>`
- `distribution::placement_points_to(target_path: &Path, source_path: &Path) -> AppResult<bool>`

### 3. Contracts
- managed identity 查询必须覆盖 `skill_project_distributions` 中 `role = 'target'` 且有 `target_path` 的 `project_custom` Project Skill。
- Inventory 只能在实际 target path 是链接且 `canonicalize(target_path) == canonicalize(canonical_path)` 时隐藏；不能仅凭数据库 target path 隐藏。
- canonical source 已删除时，placement 匹配返回 `false`；NotFound 不应阻断整个 inventory 扫描，其他 IO 错误仍须传播。

### 4. Validation & Error Matrix
- target 不是链接或链接目标不存在 -> `placement_points_to = Ok(false)`。
- target 指向不匹配 source -> `Ok(false)`，Inventory 保留该 symlink。
- source canonicalize 发生非 NotFound IO 错误 -> `AppError::Io`，不得静默隐藏。

### 5. Good/Base/Bad Cases
- Good: source Project、其他 Project 及多个 Agent 的 managed placements 均由关系查询返回并按实际 source 匹配隐藏。
- Base: 未被 Distribution 或 Sync task 管理的项目 symlink 继续出现在列表中。
- Bad: 只比较 target path，用户将 managed placement 替换为其他 source 后仍被隐藏。

### 6. Tests Required
- `crates/nexus-core/tests/project_symlink_inventory.rs`：覆盖 source/other Project、多 Agent、replacement、stale source 与 unrelated symlink。
- 保留既有普通 Skill/Prompt Distribution 与 Sync task managed link 的隐藏回归测试。

### 7. Wrong vs Correct
#### Wrong
```rust
// 仅凭记录的 target path 隐藏，无法发现被替换的链接。
managed_targets.contains(&target_path)
```

#### Correct
```rust
let mut managed = false;
for (source, target) in managed_targets {
    if target == actual_target && placement_points_to(actual_target, source)? {
        managed = true;
        break;
    }
}
```

## Scenario: Sync Task Group display state

### 1. Scope / Trigger
- Trigger: 持久化用户自定义 Sync Task Group 的展开/折叠状态，或新增读取/写入 `task_groups` 展示字段。

### 2. Signatures
- DB：`task_groups.collapsed INTEGER NOT NULL DEFAULT 0`，schema v20；Group 与 Task 的顺序仍分别使用 `task_groups.sort_index` / `tasks.sort_index`。
- Core：`TaskGroup { id, name, collapsed, tasks }`。
- Service：`set_task_group_collapsed(group_id: String, collapsed: bool) -> AppResult<TaskGroup>`。
- Tauri command：`set_task_group_collapsed(group_id: String, collapsed: bool) -> AppResult<TaskGroup>`。

### 3. Contracts
- `0` / `false` 表示展开，`1` / `true` 表示折叠；新建及 v19 迁移数据默认展开。
- 读取用户 groups 时必须 SELECT `collapsed` 并通过 serde `camelCase` 返回 `collapsed`。
- 写入用户 group 必须使用 `WHERE id = ?1 AND system_kind IS NULL`，成功后返回包含完整 tasks 的 TaskGroup。
- Session Backup 不通过该字段控制，继续由独立 UI section 管理。

### 4. Validation & Error Matrix
- 空白 group id -> `required_trimmed` validation error。
- 不存在或 `system_kind IS NOT NULL` 的 group -> `Validation("task group not found")`。
- 合法用户 group -> 更新 `collapsed` 与 `updated_at`，返回最新完整 group。

### 5. Good/Base/Bad Cases
- Good：`UPDATE task_groups SET collapsed = ?2 ... WHERE id = ?1 AND system_kind IS NULL`，前端 mutation 成功后替换完整 cache group。
- Base：旧数据库迁移后所有 group `collapsed = 0`，新 group 默认展开。
- Bad：把 Group 折叠状态放入 `settings` 或 `tasks` 的重复字段，导致 group 删除/读取链路出现第二份状态。

### 6. Tests Required
- `crates/nexus-core` schema test：v19 -> v20 后 `collapsed` 存在且默认 0。
- `crates/nexus-core/tests/sync_service.rs`：默认展开、折叠/展开 round-trip、未知 group 和系统 group 拒绝。
- 前端：`TaskGroup` 类型/Session Backup 映射测试，以及 Sync 页面 cache 乐观更新与错误回滚覆盖。

### 7. Wrong vs Correct
#### Wrong
```rust
// 只按 id 更新，可能改到 Session Backup 系统组。
conn.execute("UPDATE task_groups SET collapsed = ?2 WHERE id = ?1", params![id, collapsed])?;
```

#### Correct
```rust
let affected = conn.execute(
    "UPDATE task_groups SET collapsed = ?2, updated_at = ?3
     WHERE id = ?1 AND system_kind IS NULL",
    params![group_id, collapsed, now],
)?;
if affected == 0 {
    return Err(AppError::Validation("task group not found".to_string()));
}
```
