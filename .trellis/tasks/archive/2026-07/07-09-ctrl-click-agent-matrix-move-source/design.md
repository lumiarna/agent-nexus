# 技术设计：Ctrl 点击 Agent Matrix 移动 source

## 范围

覆盖 `agent` source kind 的 Skill 与 Prompt Agent Matrix。Project custom Skill 的 sourceless matrix 不进入移动 source 语义，继续只表达 placement target/none toggle。

## 前端交互

- `AgentIcon` / `AgentMatrixCells` 将 click event 传给上层：`onToggle(agent, event)`。
- `SkillPage.toggleCell` / `PromptPage.toggleCell` 判断 `event.ctrlKey`：
  - 普通点击：保持现有 `setSkillTarget` / `setPromptTarget` 行为。
  - Ctrl 点击当前 source：无破坏性操作，直接 return 或温和 toast。
  - Ctrl 点击非 source：调用新增 move-source mutation。
- `sourceless` matrix（Project custom Skill）不触发 move-source；Ctrl 点击按现有 placement toggle 处理，避免无 source 行被错误赋予 Agent source。

## 后端 API / IPC

新增两组命令与 typed API：

- `move_skill_source(input: MoveSkillSourceInput) -> AppResult<Skill>`
  - `skillId: string`
  - `agent: AgentName`
- `move_prompt_source(input: MovePromptSourceInput) -> AppResult<Prompt>`
  - `promptId: string`
  - `agent: AgentName`

Tauri command 仍只做透传：接收 input → 调用 `nexus-core` service → 返回 `AppResult<T>`。

React Query mutation 成功后替换对应单行 cache；Prompt/Skill scan query 仍是 server-state 真相源。

## Core 领域规则

### Skill

`SkillService::move_skill_source` 只允许 `source_kind = 'agent'`：

1. 校验 skill 存在、目标 Agent 有 skill surface、目标 Agent 不是当前 source。
2. 基于现有 canonical path 与目标 Agent 计算新 canonical path：
   - global 或 project scope 用现有 `target_path_for_parts('agent', scope, project_path, canonical_path, target_agent)`。
   - 文件夹名保持不变。
3. 若目标当前是 managed target placement，先移除该 placement；若目标路径存在非托管内容，失败且不覆盖。
4. 将 canonical source 目录移动到新 canonical path。
5. 将旧 source Agent 写成 `target`，target_path 为旧 canonical path，并创建 managed directory link 指向新 canonical path。
6. 更新 `skills.canonical_path`、`skills.source_agent`、`updated_at`。
7. 重建/修正 `skill_distributions`，保证恰好一个 `source`，原 target/none 尽量保留，旧 source 为 `target`。

### Prompt

`PromptService::move_prompt_source` 与 Skill 同构，但使用文件 placement：

1. 校验 prompt 存在、目标 Agent 支持 prompt，目标 Agent 不是当前 source，且该 scope 下可计算目标 path。
2. 目标当前为 managed target 时先移除；目标路径存在非托管内容则失败。
3. 将 canonical prompt 文件移动到目标 path。
4. 旧 source Agent 变为 `target`，target_path 为旧 canonical path，并创建 managed file link 指向新 canonical path。
5. 更新 `prompts.canonical_path`、`updated_at`。
6. 修正 `prompt_distributions`，保证恰好一个 `source`。

## 兼容性与风险

- 不新增数据库表；只新增 service 方法和 IPC command。
- 文件系统移动与 DB 更新跨边界，需保留回滚思路：若 DB 写入失败，尽量把 moved canonical path 移回旧 path，并清理已创建的旧 source placement。
- 不覆盖用户真实文件/目录；目标路径冲突应返回 validation/IO 错误。
- Project Prompt 的 target path 依赖 source/target prompt stem；移动 Generic Agent ↔ Claude Code 后，旧 canonical path 正好成为旧 source Agent 的 target path。

## 验证重点

- 普通 target toggle 不回归。
- Skill global/project source move 后：目标 Agent 唯一 source，旧 source 为 target，旧 target link 被替换为真实 canonical dir。
- Prompt global/project source move 后：目标 Agent 唯一 source，旧 source 为 target，文件 link 指向新 canonical file。
- Project custom Skill sourceless matrix Ctrl 点击不产生 Agent source。
