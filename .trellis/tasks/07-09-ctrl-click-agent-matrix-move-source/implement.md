# 实施计划：Ctrl 点击 Agent Matrix 移动 source

## 1. Backend core

1. 在 `crates/nexus-core/src/services/skills.rs` 新增：
   - `MoveSkillSourceInput { skill_id, agent }`
   - `SkillService::move_skill_source(input) -> AppResult<Skill>`
   - 复用 `target_path_for_parts`、`create_managed_directory_link`、`remove_managed_directory_link_if_present`。
2. 在 `crates/nexus-core/src/services/prompts.rs` 新增：
   - `MovePromptSourceInput { prompt_id, agent }`
   - `PromptService::move_prompt_source(input) -> AppResult<Prompt>`
   - 复用 `prompt_target_path`、`create_managed_file_link`、`remove_managed_file_link_if_present`。
3. 重点处理文件系统顺序：
   - 目标是已有 managed target：先 remove target link。
   - 目标存在非托管内容：失败，不覆盖。
   - move canonical 后创建旧 source target link。
   - DB 更新失败时尽力回滚文件移动/新 link。
4. 增加/更新 Rust 测试：
   - Skill：Ctrl 语义对应的 service move source，原 source 变 target，唯一 source。
   - Prompt：global 或 project prompt move source，原 source 变 target，唯一 source。
   - Project custom Skill：move 命令拒绝 `source_kind = project_custom`。

## 2. Tauri command

1. `src-tauri/src/commands/skills.rs` 暴露 `move_skill_source`。
2. `src-tauri/src/commands/prompts.rs` 暴露 `move_prompt_source`。
3. `src-tauri/src/lib.rs` 注册到 `generate_handler!`。

## 3. Frontend API / query

1. `src-react/src/lib/api/skills.ts` 增加 `MoveSkillSourceInput` 和 `skillsApi.moveSource`。
2. `src-react/src/lib/query/skills.ts` 增加 `useMoveSkillSourceMutation`，成功后替换单行 skill cache。
3. `src-react/src/lib/api/prompts.ts` 增加 `MovePromptSourceInput` 和 `promptsApi.moveSource`。
4. `src-react/src/lib/query/prompts.ts` 增加 `useMovePromptSourceMutation`，成功后替换单行 prompt cache。

## 4. Frontend interaction

1. `src-react/src/components/ui/agent-icon.tsx`：
   - `AgentIconProps.onClick` 接收 `React.MouseEvent<HTMLSpanElement>`。
   - `AgentMatrixCells.onToggle` 接收 `(agent, event)`。
   - tooltip 可补充 Ctrl 点击提示，但不要破坏 sourceless 文案。
2. `SkillPage.toggleCell(skill, agent, event)`：
   - `event.ctrlKey && !isProjectCustomSkill(skill)` 且非 source → 调用 move source。
   - `event.ctrlKey && isProjectCustomSkill(skill)` → 保持现有 target toggle 或温和 toast；本计划采用保持现有 toggle，满足“不参与移动 source”。
   - 普通点击保持现状。
3. `PromptPage.toggleCell(prompt, agent, event)`：
   - Ctrl 点击非 source → 调用 move source。
   - 普通点击保持现状。

## 5. 验证命令

- 前端：`cd src-react && pnpm typecheck`
- Rust 格式：优先针对改动文件 `cargo fmt --all -- <path>`；最终可跑 `pnpm rust:fmt`
- Rust 测试（Windows 注意 SQLite）：`pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`

## 6. 回滚点

- 若 move-source 文件系统语义风险过大，回滚新增 service/command/API，前端 Ctrl 点击退回 toast 提示，不改变普通点击路径。
