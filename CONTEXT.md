# Agent Nexus

`Agent Nexus` 是一个面向个人 AI 编程环境的资产中枢：它不把每个 Agent 当成彼此独立的孤岛来管理，而是把 `Provider / Project / Skill / Prompt / Session` 视为一组可复用、可分发、可观测的共享资产。

## Product

**Agent Nexus**:
一个围绕 `shared assets`、`Project` 和 `Sync` 组织的本地工作台。它把多个 agent 当作资产的消费端，而不是把产品本身建模成按 agent 切开的配置面板。
_Avoid_: Multi-agent config app, agent launcher, sync client

**Agent**:
一种可消费 `Skill`、`Prompt` 或其他共享资产的 AI 编程工具消费端。领域与 UI 显示名使用完整工具名；MVP 中 canonical agent 名为 `Generic Agent`、`Claude Code`、`CodeX`、`Copilot`、`OpenCode`。短标识如 `claude`、`opencode` 只作为实现层 ID，不作为领域显示名。`OpenCode Go` 不是 `Agent`，而是 `Provider quota` 观测入口。
_Avoid_: Model, provider, account, Claude, OpenCode Go as agent

**Asset**:
在本项目中，`Asset` 指被系统识别、展示、关联、传播或观测的领域对象，而不是任意文件副本。当前核心 `Asset` 包括 `Skill`、`Prompt`、`Session`、`Project` 和 `Provider`；但并非所有 `Asset` 都可传播。
_Avoid_: File, record, resource

**Distributable Asset**:
一种可以从 `canonical source` 建立 `Distribution` 并落到其他 agent 消费端的 `Asset`。MVP 中主要包括 `Skill` 和 `Prompt`，不包括 `Project`、`Provider` 或 `Session`。
_Avoid_: Every asset is distributable, backup item, global resource

**Archivable Content**:
一种以搜索、归档和恢复为核心价值的内容型 `Asset`。MVP 中典型对象是 `Session`；它可以进入 `Push` / `Pull`，但不进入 `Agent Matrix` 传播模型。
_Avoid_: Distributable skill, prompt placement, generic file task

**Shared Asset**:
一个应在多个 agent 或多个工作区之间复用的 `Asset`。它强调“共享关系”而不是“归属于单一 agent 的私有配置”。
_Avoid_: Per-agent config, local-only file

## Workspace

**Project**:
一个被 Agent Nexus 收录的 Git repository root。它是 `Session`、`project Skill` 和 project-bound `Sync` 任务的上下文边界，而不是任意目录；当扫描发现仓库路径变化但 `Project Key` 相同时，应续认为同一个 `Project`，而不是创建新项目。
_Avoid_: Folder, workspace path, arbitrary directory

**Project Path**:
`Project` 当前解析到的本地仓库路径。它是可变属性，不构成 `Project` 身份；当仓库移动或重命名时，系统可以在 `Project Key` 不变的前提下更新它。
_Avoid_: Stable identity, project key

**Stale Project**:
一个已经存在 `Project` 记录、但当前 `Project Path` 不存在，且扫描阶段未能续认到同 `Project Key` 新路径的项目状态。它应被视为待处置状态，而不是立即删除的脏数据。
_Avoid_: Auto-deleted project, healthy project, identity loss

**Project Status**:
`Project` 在列表中的一级可见状态。MVP 中只包括 `active`、`stale` 和 `hidden`；其中 `moved`、`renamed` 等仅作为事件或提示，不提升为一级状态。
_Avoid_: Event-as-status, overly granular state machine

**Git Base Folder**:
用于自动发现 `Project` 的扫描根目录。它不是 `Project` 本身，只是 `Project discovery` 的输入范围。
_Avoid_: Project, workspace

**Project Key**:
用于跨设备归并同一 `Project` 的稳定身份键。MVP 中它默认取项目目录名，但 UI 不提供编辑入口；目录名只是默认值，不应被等同为身份语义本身。未来若支持显式覆写或迁移，应通过受控流程维持同一 `Project` 身份，而不是把目录改名天然视为新项目。它用于 `Session` 的 WebDAV 归档与聚合。
_Avoid_: Local path, mutable folder name as identity, remote guess

## Assets

**Skill**:
一种可被 agent 消费的共享能力资产。`Skill` 同时支持 `global` 与 `project` 两种 `Scope`，并以 `canonical source` 作为资产身份。其 canonical source 有两种来源：`agent`（落在某个 Agent 的固定 skills 目录，由 `Source Agent` 拥有）与 `project_custom`（落在 `Project Custom Source`，不归属任一 Agent）。
_Avoid_: Skill copy, skill target, generated placement

**Project Custom Source**:
一个由 `Project` 维护、不归属任一 `Agent` 的 `Skill` canonical source。它来自 Project 配置的 `custom_skills_dirs`，与固定 Agent project skills 目录并列扫描；它是合法 canonical source，但不是 `Agent source`。其 Global / 其他 Agent 落点都只是 managed `Placement`，不构成新的 canonical Skill。
_Avoid_: Agent source, new global skill copy, placement-as-source

**Prompt**:
一种面向 agent 的共享提示资产,同时支持 `global` 与 `project` 两种 `Scope`。Global Prompt 使用各 Agent 的全局提示文件;Project Prompt 默认覆盖仓库根的 `AGENTS.md`(Generic Agent)与 `CLAUDE.md`(Claude Code),并可由 Project 的 `extra_prompt_files` 列表扩展同名命名空间下的额外文件(`AGENTS*.md` / `CLAUDE*.md`)。
_Avoid_: Arbitrary project prompt path, prompt copy

**Extra Prompt File**:
Project 通过 `extra_prompt_files: Vec<String>` 显式登记的额外 Prompt 文件,每条是相对项目根的路径。它必须匹配某个 Agent 的 `projectPromptFile` glob(例如 `AGENTS*.md` 对应 Generic Agent、`CLAUDE*.md` 对应 Claude Code),由该 Agent 担任 Source Agent;不匹配则拒绝入列。它与 Skill 的 `Project Custom Source` 故意区分——不创建新 canonical source,只是在 Agent 既有命名空间内扩大扫描范围。
_Avoid_: Custom prompt, additional prompt, project prompt override, project prompt custom source

**Session**:
一种可搜索、可归档的会话内容资产。它既有 `Local` 视图，也有 `Cloud` 视图，但归档与恢复的任务粒度保持在 `Project` 级。
_Avoid_: Chat log only, backup artifact only

**Provider**:
一个提供 quota 信息与 credential source 的外部服务身份。`Provider` 在本项目中是全局资源，不做 project-level quota 归因。部分 `Provider` 与 `Agent` 共享展示事实，例如 `Claude Code`、`CodeX`、`Copilot`；部分 `Provider` 不是 `Agent`，例如 `OpenCode Go`。同一品牌下的 `Agent` 与 `Provider` 不自动等价：`OpenCode` 是 agent 消费端，`OpenCode Go` 是 quota provider。
_Avoid_: Project provider, account manager, OpenCode as OpenCode Go, provider implies agent

**Provider Connection Params**:
为 `Provider` 的 quota 观测补充的连接参数，例如 `OpenCode Go` 的 workspace identifier 或请求所需材料。它服务于观测能力，不表示 Agent Nexus 接管第三方身份生命周期。
_Avoid_: Account ownership, login manager, credential lifecycle

**Provider Display Preferences**:
用户对 `Provider` 展示方式的显式偏好，例如卡片排序、隐藏与次级表面显示选项。它影响界面呈现，不改变 `Provider` 身份或 quota 语义。
_Avoid_: Provider identity, quota data, credential config

**Surface Preference**:
针对某个具体展示表面的可见性或呈现偏好。它从属于特定 surface，例如 Windows 任务栏，不应与 `Provider` 的全局观测配置混为一体。
_Avoid_: Global provider identity, connection params

**Card Visibility**:
`Provider` 在主页面卡片视图中的显示偏好。它只影响该主页面 surface，不自动影响其他 surface 的显示与否。
_Avoid_: Global hide, tray visibility

**Tray Metric Mode**:
Windows 任务栏对 `Provider quota` 采用的统一主展示口径。MVP 中它在整个任务栏范围内全局统一配置，而不是按 `Provider` 分别覆盖。
_Avoid_: Per-provider tray metric, mixed tray semantics

## Identity

**Canonical Source**:
一个 `Distributable Asset` 的权威来源位置。只有 `canonical source` 才算该可传播资产本体，所有 `symlink` 或 `copy` 产生的目标都只是 `placement` 或 `target`。
_Avoid_: Copy target, mirrored file, duplicate asset

**Placement**:
一个已知由系统管理、并与某个 `canonical source` 相关联的落点。它表示资产被放到了哪里，而不是产生了一个新资产。
_Avoid_: New asset, cloned asset

**Scope**:
`Asset` 的适用范围。当前用于 `Skill` 与 `Prompt`，取值为 `global` 或 `project`。
_Avoid_: Type, category

## Distribution

**Distribution**:
把一个 `Distributable Asset` 从 `canonical source` 传播到其他消费端的单向关系（Local → Local）。它强调关系建立与目标落点，而不是双向同步。
_Avoid_: Two-way sync, replication mesh

**Agent Matrix**:
`Skill` 和 `Prompt` 页面中的传播关系模型。它表达某个资产在不同 agent 上的 `source / target / none` 关系。对 `agent` source kind 的行，每一行必须且只能有一个 `source`；对 `Project Custom Source` 的 `Skill` 行没有 `source` cell，cell 只表达 `target / none` 的 Global placement，行内改以 `Project source` 标识来源。MVP 中该模型可以用紧凑行与 Agent 图标组呈现，而不要求固定采用每个 agent 单独占一列的显式宽矩阵。
_Avoid_: Agent tree, config table, task grid, every row has an agent source

**Source Agent**:
在 `Agent Matrix` 中拥有该资产 `canonical source` 的 agent，仅当 source kind 为 `agent` 时存在。它表示关系起点，而不是运行时执行者；`Project Custom Source` 的 `Skill` 没有 `Source Agent`。
_Avoid_: Owner, primary target, source of project custom skill

**Target Agent**:
在 `Agent Matrix` 中接收资产传播关系的 agent。对 `Skill` 和 `Prompt`，其目标路径由系统按 agent 与上下文自动计算。
_Avoid_: Secondary source, duplicate source

**Agent Capability Surface**:
某个 `Agent` 在当前产品中实际参与的资产与页面范围。它集中描述 canonical order、配置根、`Skill` surface（global/project skill 目录）、`Prompt` surface（global prompt 文件与可选的 project prompt 文件）以及可选的 `Provider` 展示事实。Global Prompt 覆盖全部 prompt-capable agent；Project Prompt 只开放 Generic Agent 的 `AGENTS.md` 与 Claude Code 的 `CLAUDE.md`。`Agent` 的领域身份可以完整存在，但其可操作 surface 可以分阶段开放；当前已确认 `Copilot` 在 MVP 中参与 `Skill` 与 Global Prompt。`Agent Capability Surface` 的展示名必须使用 canonical agent 名；内部 ID（如 `claude`、`opencode`）只能作为实现层标识，不能替代 `Claude Code`、`OpenCode` 等领域名。
_Avoid_: Partial agent identity, ad hoc special case, short ID as display name, OpenCode Go as agent surface

## Sync

**Location Type**:
`Task` 的 source 或 target 所处的位置类型。取值为 `Local` 或 `Cloud`。UI 层统一使用 `Cloud`；`WebDAV` 属于实现层术语。
_Avoid_: WebDAV (in UI), path prefix as type

**Direction**:
`Task` 的传播方向，由 source 与 target 的 `Location Type` 自动派生：Local→Local = `Distribution`，Local→Cloud = `Push`，Cloud→Local = `Pull`。Cloud→Cloud 非法。用户不手选此值。
_Avoid_: User-selected direction, manual label

**Action**:
`Task` 执行时对目标的写入方式。取值为 `Symlink` 或 `Copy`。`Symlink` 仅在 Direction 为 `Distribution` 时可用；`Push` / `Pull` 方向时 `Symlink` 不可选。
_Avoid_: lowercase symlink/copy, transfer mode

**Sync**:
Agent Nexus 的任务与预设工作域。它统一承载 `Distribution`、`Push` 和 `Pull` 的底层执行与可观测性，但不承担资产主视图。MVP 中它以自定义 `Task Group` / `Task` 工作台为主，系统默认任务降为次级观察区。`Push` 与 `Pull` 是两个显式的单向任务方向，不合并成一个双向任务。
_Avoid_: Asset page, file manager, two-way sync engine

**Sync Task**:
一个具体的单向执行任务，严格遵守 `1 source → 1 target` 规则。它是运行对象，不是模板，也不是资产本体。`Direction` 由 source 与 target 的 `Location Type` 自动派生，不由用户手选。`Task` 可独立配置 `Manual` 或 `Schedule` 调度，并由 Agent Nexus 内建执行。
_Avoid_: Template, relationship only, background daemon, task group as execution type, multi-target task

**Task Group**:
一个包含一个或多个 `Sync Task` 的组织与编排容器。它用于创建、排序、批量查看与批量触发，但不承载 `Distribution` / `Push/Pull` 等执行方向语义。MVP 中 `Create custom task` 的默认创建单位是 `Task Group`，单任务只是单元素 group。
_Avoid_: Execution type, workflow engine, task type

**Template**:
用于快速创建 `Task Group` 的预设定义。Project-bound 模板可使用 `{{project_dir}}`（当前本地 Project Path）与 `{{project_key}}`（跨设备稳定 Project Key）；模板物化为 `Task` 后复用相同执行器。自定义模板实例化后不回流控制既有任务，系统托管模板可在 Project Path 变化后重新物化路径并保留运行状态。
_Avoid_: Task, live config, inherited instance

**Instance**:
由 `Template` 或手工操作创建出的具体 `Task Group` 或 `Sync Task` 实例。它创建后独立存在，不自动跟随模板变更。
_Avoid_: Template copy with inheritance, managed child config

**Generic File Distribution/Backup**:
一种面向通用文件或目录的 `Sync` 类型。它比 `Skill Distribution` 和 `Prompt Distribution` 更通用，但仍严格遵守同一套单向同步规则。
_Avoid_: Escape hatch, unrestricted sync

**Skill Distribution**:
围绕 `Skill` 传播关系生成和维护的 `Sync` 类型。用户主要通过 `Agent Matrix` 定义关系，而不是直接在 `Sync` 中编辑其核心语义。
_Avoid_: Generic symlink task, manual sync relation

**Prompt Distribution**:
围绕 `Prompt` 传播关系生成和维护的 `Sync` 类型。它和 `Skill Distribution` 一样是高层关系的底层承载，而不是主编辑入口。
_Avoid_: Prompt sync config editor

**Session Backup**:
围绕 `Session` 归档建立的系统托管 `Sync` 类型。每个 `Project` 默认物化一个 Copy Task：Source `Local {{project_dir}}/__sessions/`、Target `Cloud Session/{{project_key}}/`、Schedule `0 * * * *`。Source、Target 与 Action 由系统维护；Schedule 首次采用默认值，之后允许用户逐 Task 调整。UI 复用 Task Group，可运行单 Task 或 Run Group，但不允许新增、删除或排序系统 Task。它负责把本地 `Session` 目录归档到 Cloud，而不是管理会话内容本身。
_Avoid_: Session viewer, session search

**Push**:
从本地到 Cloud 的单向操作。语义中性，只表达方向，不携带镜像/删除等隐含策略。
_Avoid_: Mirror sync, two-way sync, Backup

**Pull**:
从 Cloud 到本地的单向操作。语义中性，只表达方向，不等于双向同步。
_Avoid_: Reverse sync, Restore

## Session Views

**Local Session**:
来自当前设备、已收录 `Project` 的本地会话集合。它是 `Session` 页在 `Local` 来源下的真相源。
_Avoid_: Cloud cache, merged archive

**Cloud Session**:
来自 WebDAV 汇总归档的会话集合。它代表跨设备聚合后的会话视图，是 `Session` 页在 `Cloud` 来源下的真相源。这里的聚合是只读视图聚合，不表示多源合并同步、冲突解决或远端默认成为通用 source。`WebDAV` 属于设置与实现层术语；在主内容界面中应优先使用 `Cloud`。
_Avoid_: Local session, mixed source view

**Session Directory**:
一个 `Project` 的本地会话目录。默认模板是 `{{project_dir}}/__sessions/`，可被 project-level override 覆写为单值字符串。每个 `Project` 始终只有一个 `Session Directory`——这是有意识的约束,不是 MVP 临时限制:Session 是 `Archivable Content`,没有 Distributable Asset 那样的"独立 source"概念可对应,多 dir 不会带来新的可观测维度,只会增加合并扫描的歧义;UI 编辑入口仅在 Project 详情 Session card 暴露。
_Avoid_: Multi-root session source, fixed hardcoded path, session source vector

## Boundaries

**Credential Visibility**:
对 `Provider` 凭据来源与可用性的展示能力。它只负责发现与诊断，不接管第三方身份生命周期。
_Avoid_: Login flow, account management, token writer

**Quick Action**:
在 `Session` 或 `Provider` 等上下文页中就近提供的轻量动作。它缩短操作路径，但不替代 `Sync` 的完整任务定义能力。
_Avoid_: Full configuration surface, task editor

**Display Order**:
一种用户可显式调整的列表顺序偏好。MVP 中 `Provider` 卡片、`Project` 列表、`Task Group` 列表以及 `Task Group` 内的 `Task` 列表都支持拖拽排序；而 agent 相关展示顺序则采用固定 canonical order：`Claude Code / CodeX / Copilot / OpenCode`。
_Avoid_: Implicit heuristic order, mixed agent order

**Global Resource**:
不应被 project-level 归因或切分的资源。当前 `Provider quota` 是典型的 `Global Resource`。
_Avoid_: Project quota, per-repo quota
