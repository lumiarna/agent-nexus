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

## 表和字段命名

- 表名和字段名对齐 `CONTEXT.md` 领域词：`projects`、`skills`、`prompts`、`providers`、`session_index`、`task_groups`、`tasks`、`skill_distributions`、`skill_project_distributions`（跨 Project 投影，见下文 Scenario）。
- 时间戳使用 Unix epoch seconds；主键多数为 UUID TEXT。
- JSON/text list 字段只用于弱结构或简单配置，例如 `connection_params`、换行分隔的 `custom_skills_dirs` / `extra_prompt_files`。

## Scenario: Cross-project `project_custom` Skill distribution

### 1. Scope / Trigger
- Trigger: 为 `source_kind = project_custom` 的 Skill 增加跨 Project 传播（落到其他 Project 默认 Agent 的 fixed project skills dir），或改动 `skill_project_distributions` 表 / 关联 service / scan reconcile。

### 2. Signatures
- DB 表（`migrate_to_v19`）：
  ```sql
  CREATE TABLE skill_project_distributions (
    skill_id TEXT NOT NULL,
    target_project_id TEXT NOT NULL,
    agent TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('target','none')),
    target_path TEXT,
    CHECK ((role='target' AND target_path IS NOT NULL) OR (role='none' AND target_path IS NULL)),
    PRIMARY KEY (skill_id, target_project_id, agent),
    FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
    FOREIGN KEY (target_project_id) REFERENCES projects(id) ON DELETE CASCADE
  );
  ```
- Service `crates/nexus-core/src/services/skills.rs`：
  - `set_project_skill_project(SetProjectSkillProjectInput) -> AppResult<Vec<Skill>>`
  - `set_project_skill_target(SetProjectSkillTargetInput) -> AppResult<Vec<Skill>>`
  - `project_target_path_for_skill(target_project_path, canonical_path, agent) -> AppResult<PathBuf>`
  - `reconcile_project_distributions()`（scan 后回收集已断链的 target row）

### 3. Contracts
- 仅 `skills.source_kind = 'project_custom'` 可写入 `skill_project_distributions`（service 在 `project_skill_context` 校验）。
- `target_project_id` 可以等于 canonical skill 的 `project_id`（source/current Project target）；目标 Project 必须存在且 `status = 'active'`。
- 落点用目标 Agent `skill.project_dir`（fixed project skills dir），**绝不**用目标 Project `customSkillsDirs`；`agent.project_dir` 不可用时失败。
- 托管链接复用 `create_managed_directory_link` / `remove_managed_directory_link_if_present`；目标路径已存在真实目录/非托管文件即失败，不覆盖、不合并、不改名。
- scan 重建 canonical sources 时，`discover_skill_sources` 跳过 symlink/junction，跨 Project placement 不会被误识别为 canonical source；scan 只会 reconcile `skill_project_distributions` 中断链的 target row。
- `Skill` DTO 在 projection 行上用 composite display id `{skill_id}::project::{target_project_id}`，并带 `canonical_skill_id` 指向真实 backend id（前端 mutation 必须 canonical id，见 `agent-nexus/frontend/type-safety.md` 的同主题 Scenario）。

### 4. Validation & Error Matrix
- skill 不存在 -> `Validation("skill was not found")`
- `source_kind != project_custom` -> `Validation("only Project custom Skills can be propagated to Project targets")`
- canonical skill 无 `project_id` -> `Validation("skill has no source Project")`
- target Project 不存在/非 active -> `Validation("target project was not found or is not active")`
- Agent 无 skill surface -> `Validation("<agent> does not support skill placement")`
- 目标路径被预占 -> managed link 创建失败，原内容保留

### 5. Good/Base/Bad Cases
- Good: 源 Project custom Skill 传播到当前/source Project 或另一 active Project，目标默认 Agent project skills dir 出现托管链接，`list_skills` 返回该目标 Project 的 projection row。源侧取消时该目标 Project 全部 Agent placement 与目标 row 一并清除。
- Base: 只传播到 Global 时走既有 `skill_distributions`，与 v19 改动无关。
- Bad: 把 placement `target_path` 落到目标 `customSkillsDirs` -> rescan 会把它当新 canonical source，偷换 canonical 身份。

### 6. Tests Required
- `crates/nexus-core/tests/skill_service.rs`：
  - `propagates_project_custom_skill_to_global_and_keeps_single_source` 不回归
  - `propagates_project_custom_skill_to_other_project` assert target_path 指向目标默认 Agent fixed project dir
  - `target_project_incoming_row_fans_out_and_disappears` assert cells 无 source、末位移除后行消失
  - `cancelling_target_project_removes_all_its_placements` assert 删除该 target Project 全部 placements
  - `cross_project_placement_does_not_become_canonical_on_rescan`
  - `cross_project_propagation_fails_when_target_path_exists`
  - `cross_project_propagation_rejects_agent_sourced_skill`

### 7. Wrong vs Correct
#### Wrong
```rust
// 把跨 Project placement 落到 customSkillsDirs
let target = resolve_custom_dir(&target_root.path, custom_dir)?.join(dir_name);
// 或：target_project_id 不校验直接写
self.db.execute("INSERT INTO skill_project_distributions ...")
```

#### Correct
```rust
let target_root = self.project_root(&target_project_id)?;
let target_path = project_target_path_for_skill(&target_root.path, &context.canonical_path, default_agent)?;
// 校验 source_kind=project_custom + target!=source + Agent skill-capable，再走 write_target
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
