# Project 自定义源按资产形态各异,不强行"对齐"

Skill 的 `customSkillsDirs: Vec<String>` 已经支持每 Project 添加独立的项目级 canonical source(每条 dir 独立发现 `SKILL.md`,无 `Source Agent`,可 Propagate 到 Global)。当我们审视"Prompt 和 Session 也应该支持自定义目录"时,先后被反向挑战推翻:(1) Session 是 `Archivable Content` 不是 `Distributable Asset`,硬套多 dir 是负债;(2) Prompt 的 extras 不应该污染进 ignore / settings.json 等非 Prompt 概念。最终决定:**三个资产各自尊重物理约束**,Skill 沿用 `customSkillsDirs: Vec<String>`、Prompt 新增 `extraPromptFiles: Vec<String>` 显式列表、Session 仅补 `sessions_dir` 单值编辑 UI;它们的"对齐"仅在 UI 模式(每个 card 头部一个 configure 按钮 + 独立 modal),不在数据形态。这条决策有意拒绝把三者硬塞进同一个数据形状,代价是用户需要分别理解三种配置语义,收益是不产生伪领域概念与未来 schema 债务。

## Status

accepted (2026-06-27)

## Considered Options

- **Prompt 用约定驱动(glob 自动发现)**:`AGENTS*.md` / `CLAUDE*.md` 自动识别为 extras,零 schema 零 UI。否决理由:用户难以审计"哪些文件算 Prompt";破坏"对齐 Skill"的心智一致性;与 `customSkillsDirs` 的显式语义偏离。
- **Prompt 用混合(约定 + UI disable)**:`AGENTS*.md` 自动发现,UI 可 disable 某些文件。否决理由:Simplicity First,两个机制叠加反而难以调试;disable 列表与显式列表在数据上很难区分。
- **Session 引入多 `sessions_dir`(Vec 化)**:`Session` 突破 CONTEXT.md 的 "MVP only one Session Directory" 约束。否决理由:Session 是 `Archivable Content`,没有 distribution 模型,"附加源"对它没语义;真实需求大概率只是改默认路径。
- **Session 引入 `Vec<String> custom_session_dirs + sessions_dir primary`**:在保留 primary 的同时允许附加。否决理由:同上一条,Session 不存在"独立 source"概念可对应;附加 dir 合并后无法区分来源,审计性差。
- **统一一个 "Project custom sources" modal**:三个资产的配置塞进同一个 modal。否决理由:任何一项变更都要打开整个 modal,简单操作变重;三个 feature 本就独立,耦合到 modal 是 UI 债。
- **三个独立 modal**(本决策):每个 card 头部一个按钮 + 独立 modal,与 Skill 已有 modal 模板一致。

## Consequences

- **Schema v17 新增一列**:`projects.extra_prompt_files TEXT NOT NULL DEFAULT ''`(换行分隔,与 `custom_skills_dirs` 同款);`AgentCapabilitySurface.prompt.projectFile` 由字面 filename 扩为 glob pattern(`AGENTS*.md` / `CLAUDE*.md`),向后兼容(`AGENTS.md` 仍匹配 `AGENTS*.md`)。
- **新增两个 IPC**:`set_project_extra_prompt_files`(对齐 `set_project_custom_skills_dirs` 风格)、`set_project_sessions_dir`。
- **Prompt extras 必须匹配 Agent glob**:UI 输入校验 + 后端 scan 时校验,不匹配拒绝入 DB。这条约束显式记录,避免误把 `.claudeignore` / `settings.json` 等塞进 Prompt。
- **Prompt extras 继承 Source Agent**:与 Skill 的 `Project Custom Source`(无 Source Agent)语义刻意区分;不引入"Project Prompt Custom Source"词条。
- **Session 维持单值语义**:`sessions_dir` 始终是 `String`;UI 仅补编辑入口,**不改 Vec**。这是有意识的"不对齐"。
- **`.claudeignore` 等非 Prompt 文件划出本次范围**:它们是 ignore / settings,不是 Prompt。后续若要 track,作为独立 feature 处理,不污染 Prompt 概念。

## Implementation Notes

- Prompt scan 顺序:`AGENTS.md` / `CLAUDE.md` primary 自动发现 → `extra_prompt_files` 列表每条文件 → 各自匹配 Agent glob → 生成 Prompt 行带 Source Agent。
- **Extra Prompt File 的跨 Agent 分发用 "stem-swap"**:project prompt 落到 target Agent 的路径 = 把 source Agent 的 prompt-file stem(`AGENTS` / `CLAUDE`)换成 target Agent 的 stem,保留目录前缀与 stem 与 `.md` 之间的后缀。即 `AGENTS.md → CLAUDE.md`(后缀空,与既有 primary 行为字节一致,无回归)、`AGENTS.local.md → CLAUDE.local.md`、`docs/CLAUDE.md → docs/AGENTS.md`。这条规则是 ADR 决策时未定的语义,实现时收敛为此:它是唯一对 primary 与 extra 统一、且跨 Agent 无文件名碰撞的映射。Global scope 不走 stem-swap,仍用各 Agent 的 `global_file`。
- 命名 glob 选最宽 `AGENTS*.md` / `CLAUDE*.md`,因为只有用户显式加进 `extra_prompt_files` 的文件才被当 extra,glob 严格度不影响过滤安全性。
- Session 编辑入口放 Session card 头部(对齐 Skill "Custom skills dirs" / Prompt "Custom prompt files" 位置),不放在 Project 头部只读摘要旁。
- Project 列表页布局改为 4 列 (`drag | key+path | skill | prompt | session | ⋯`),SYNC 移出 Assets 列(它是任务数不是 Asset)。每个 asset cell 显示"大数字 + 最多 2 行小字 + `+N`"。