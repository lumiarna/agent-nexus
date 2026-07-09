# Ctrl 点击 Agent 矩阵移动 source

## 目标

用户希望在 Skill / Prompt 的 Agent Matrix 中，通过按住 Ctrl 点击某个 Agent 单元格，将当前资产的 canonical source 移动到被点击的 Agent，从而更快调整 Source Agent，而不是只能增删 target。

## 已确认事实

- `CONTEXT.md` 定义：Agent Matrix 表达资产在不同 Agent 上的 `source / target / none` 关系；agent-sourced 行必须且只能有一个 `source`。
- 前端矩阵单元由 `src-react/src/components/ui/agent-icon.tsx` 的 `AgentMatrixCells` / `AgentIcon` 渲染；当前 `onToggle(agent)` 不接收鼠标事件或修饰键。
- Skill 页面：`src-react/src/components/skill/SkillPage.tsx::toggleCell` 当前点击非 source 单元格只调用 `setSkillTarget` 增删 target；点击 source 直接 return。
- Prompt 页面：`src-react/src/components/prompt/PromptPage.tsx::toggleCell` 当前点击非 source 单元格只调用 `setPromptTarget` 增删 target；点击 source 直接 return。
- 后端现有命令只有 `set_skill_target` / `set_prompt_target`；它们明确拒绝把 source agent 当 target 切换，尚无“移动 source”的命令。
- Project custom Skill 行没有 Agent source cell；其 Agent Matrix 是 sourceless placement 矩阵，不应被“移动 Source Agent”语义影响。

## 初始需求

- R1：普通点击 Agent Matrix 继续保持现有行为：target / none 切换，不改变 source。
- R2：按住 Ctrl 点击 Agent Matrix 的非 source Agent 时，触发“移动 source 到该 Agent”的动作。
- R3：移动后该资产在 Agent Matrix 中仍必须只有一个 source。
- R4：Project custom Skill 的 sourceless 矩阵不参与移动 source；Ctrl 点击仍应按现有 placement toggle 处理或给出温和提示。
- R5：交互需要同时适配 Skill / Prompt，保证同名 Agent Matrix 的交互一致。

## 已确认产品决策

- D1：Ctrl 点击移动 source 同时覆盖 Skill 与 Prompt 的 Agent Matrix。
- D2：Ctrl 点击已有 `target` 单元格移动 source 时，原 source 变成 `target`，相当于交换 source/target 角色并保留原 Agent 继续消费该资产。

## 验收标准草案

- A1：普通点击非 source 单元格，仍只增删 target。
- A2：Ctrl 点击可作为 source 的非 source 单元格后，列表刷新显示被点击 Agent 为唯一 source，原 source 不再是 source。
- A2.1：若被点击单元格原本是 `target`，移动后原 source Agent 显示为 `target`。
- A3：Ctrl 点击当前 source 单元格不产生破坏性操作。
- A4：Project custom Skill 的 sourceless Agent Matrix 不会产生 Agent source。
- A5：相关前端 typecheck 与后端测试通过。
