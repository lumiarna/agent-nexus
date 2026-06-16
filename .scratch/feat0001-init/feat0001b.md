# Init Prototype

## 1. Product Summary

`Agent Nexus` 是一个面向个人 AI 编程环境的资产中枢：它不把每个 `Agent` 当成彼此独立的孤岛来管理，而是把 `Provider / Project / Skill / Prompt / Session` 视为一组可复用、可分发、可观测的共享资产。

本 PRD 的目标不是约束实现细节，而是提供一份足够清晰的信息架构、页面目标、交互边界与状态模型，使设计师可以基于它直接绘制原型图。

## 2. Product Goal

### Primary Goal

- 让用户在一个桌面应用中统一查看和管理自己的 AI 编程环境资产。
- 让用户不必按单个 `Agent` 分别维护 `Skill`、`Prompt`、`Session` 和相关同步关系。
- 让用户能以“共享资产”的视角完成发现、传播、归档、搜索与观察。

### User Value

- 看清 `Provider quota`
- 看清 `Project` 与共享资产的关系
- 快速把 `Skill` / `Prompt` 传播给其他 `Agent`
- 搜索 `Session`，并在 `Local` / `Cloud` 之间切换真相源
- 通过 `Sync` 管理底层任务，而不必手动维护文件关系

## 3. Target User

### Primary User

- 使用多个 AI coding agents 的个人开发者
- 同时维护多个 Git repositories
- 有跨项目复用 `Skill` / `Prompt` 的需求
- 有跨设备检索与归档 `Session` 的需求
- 希望快速查看各 `Provider` 的 quota，而不是逐个打开官网或 CLI

## 4. MVP Scope

### In Scope

- `Provider` 全局 quota 可见性
- `Project` 自动发现与手动收录
- `Skill` 的 `global` / `project` 管理与传播矩阵
- `Prompt` 的 `global` 管理与传播矩阵
- `Session` 的 `Local / Cloud` 双来源视图与搜索
- `Sync` 任务中心
- `Generic File Distribution/Backup` 的模板与实例

### Out of Scope

- `Provider` 安装、登录、账号生命周期管理
- 双向同步
- 多源合并同步
- `Provider quota` 的 `project-level attribution`
- `project-level Prompt`
- 通用内容编辑器
- 除 `disable-model-invocation` 外的 `frontmatter` 结构化编辑
- `Session` 的多目录来源
- `Template` 对实例的回流控制
- 跨 `Local / Cloud` 的混合 `Session` 搜索
- MVP 内的 `cron` / 自动调度

## 5. Platform And UI Defaults

- Platform: `Windows` + `macOS`
- Theme: `Light` / `Dark`
- Default Theme: `Light`
- Language: UI 默认英文，MVP 不做多语言
- Path display: UI 中路径尽量规范化显示为 `/`

## 6. Core Product Principles

### Shared Asset First

- 优先把对象建模为 `shared asset`，而不是按 `Agent` 各自为政的私有配置。

### Single Source, Multi Target

- 所有 `Sync` 均遵守 `single source -> multiple targets` 的单向规则。

### Strong Default, Explicit Override

- 能自动推导的优先自动推导。
- 少数例外用显式 override 处理。

### Context Page + Task Center

- 上下文页负责就近操作。
- `Sync` 页负责统一观测和完整任务入口。

### No Heuristic Magic

- 不依赖名称相似、内容相似等启发式猜测资产身份。
- 只基于系统已知关系做归并。

### Asset Types Are Not All Distributable

- `Skill` 和 `Prompt` 是 MVP 中的主要 `Distributable Asset`。
- `Session` 是可搜索、可归档的内容资产，不进入 `Agent Matrix` 传播模型。
- `Project` 与 `Provider` 是上下文或观测资源，不作为可传播资产处理。

### Backup And Pull Are Separate One-Way Tasks

- `Backup` 与 `Restore/Pull` 都是单向任务。
- 从 WebDAV 拉回本地时，应创建独立的 `Restore/Pull` 任务。
- 不通过反转既有 `Backup` 任务来模拟双向同步。

## 7. Information Architecture

一级导航顺序固定为：

1. `Provider`
2. `Project`
3. `Skill`
4. `Prompt`
5. `Session`
6. `Sync`

### Navigation Semantics

- `Provider`: 全局 quota 与 credential visibility
- `Project`: 工作区上下文与 project detail
- `Skill`: 共享能力资产与传播矩阵
- `Prompt`: 全局提示资产与传播矩阵
- `Session`: 可搜索的会话内容域
- `Sync`: 任务、预设与底层执行中心

## 8. Shared Interaction Rules

### `Provider`

- `Provider quota` 是 `Global Resource`
- 不做 `project-aware quota`
- 页面允许少量诊断型动作，例如 `Refresh now`

### `Project`

- 一个 `Project` = 一个 Git repository root
- `Git Base Folder` 只是扫描根，不是 `Project`
- `project key` 默认从 canonical Git remote 派生
- 无 remote 时可退回项目目录名
- 多 remote 且无法确定 canonical remote 时，需要用户确认或覆写
- `Project detail` 是 project-bound 操作的主要承载页

### `Skill`

- 同时覆盖 `global skill` 与 `project skill`
- `Skill` 以 `canonical source` 为资产身份
- `copy/symlink target` 只是 `placement`
- `disable-model-invocation` 是唯一支持的 `frontmatter` 编辑项

### `Prompt`

- 只覆盖 `global prompt`
- 不做 `project-level prompt`

### `Session`

- `Session` 同时支持 `Local` 与 `Cloud` 视图
- `Cloud Session` 来自 WebDAV 汇总结果
- `Cloud Session` 的汇总是只读视图聚合，不代表多源合并同步
- `Session Directory` 默认是 `${project_dir}/__sessions`
- 每个 `Project` 只允许一个 `Session Directory`

### `Sync`

- 只做单向任务
- `Backup` 与 `Restore/Pull` 是两个独立的单向任务方向
- `Skill/Prompt` 的传播关系由高层矩阵定义
- `Sync` 中可见对应记录，可执行、可观测，但不在 `Sync` 中反向改 source、target 或 action

## 9. Screen Inventory

以下页面与状态是原型设计必须覆盖的主范围。

### 9.1 App Shell

#### Purpose

- 提供全局导航与统一页面骨架

#### Required Structure

- 左侧或顶部主导航：`Provider / Project / Skill / Prompt / Session / Sync`
- 页面标题区
- 页面级 toolbar 区
- 主内容区

#### Design Guidance

- 这是一个 `desktop productivity app`，不是 marketing landing page
- 页面密度可以偏高，但层级必须清楚

---

### 9.2 `Provider` Page

#### Purpose

- 统一查看每个 `Provider` 的 quota、reset 与 credential visibility

#### Primary View

- 以 `Provider card` 为主
- 不以统一总表为主

#### Card Content

- `Provider name`
- 当前可用状态
- `plan`（若可得）
- 主要 quota 百分比
- 每个 quota window 的使用情况与 reset 信息
- `credential source`
- 错误或缺失状态

#### Quick Actions

- `Refresh now`
- `Re-check credentials`
- `Open config location` 或 `Open docs`（可选）

#### Required States

- `available`
- `no credentials`
- `token expired`
- `request failed`
- `loading`

#### Prototype Note

- 主视觉应优先服务“我还能用多少、什么时候重置”
- 不应做成安装器或登录管理页

---

### 9.3 `Project` Page

#### Purpose

- 展示所有已收录的 Git repositories，并作为 project context 的入口

#### List View Content

- `Project name`
- `repo path`
- `project key`
- `Session Directory`
- 关键计数摘要（例如 `Skill` / `Session` / `Sync`）

#### List View Actions

- `Add Git Base Folder`
- `Add Project`
- `Open Project`

#### Interaction Rule

- 列表页不承担重操作
- project-bound 的 Sync 创建主要进入 `Project detail` 后完成

---

### 9.4 `Project Detail` Page

#### Purpose

- 在单个 `Project` 上聚合本地上下文、资产关系与 project-bound 动作

#### Header Content

- `Project name`
- `repo path`
- `project key`
- `Session Directory`
- override 标记（若有）

#### Required Sections

- `Skill` summary
- `Prompt` summary（只作为引用/关联，不作为 project prompt）
- `Session` panel
- `Sync` summary

#### Session Panel

- 复用 `Session` 页面同一套组件
- 默认来源 = `Local`
- 支持切换到 `Cloud`

#### Quick Actions

- `Archive now`
- `Pull now`
- `Create Session Backup`
- `Create Generic File Task`

#### Prototype Note

- 该页应体现“Project 是上下文入口”，而不是单纯详情表单

---

### 9.5 `Skill` Page

#### Purpose

- 管理 `Skill` 资产，并通过 `Agent Matrix` 快速配置传播关系

#### Primary Structure

- 顶部 `Scope switch`: `Global / Project`
- 当 `Project` scope 被选中时，需要 `Project selector`
- 主体是 `asset list + Agent Matrix`

#### Recommended Toolbar

- `Scope switch`
- Search by `name / description`
- 可选的 `Agent summary chips`

#### Row Content

- `Skill name`
- `description`
- `Source Agent`
- `source path`
- `disable-model-invocation` toggle
- `Agent Matrix` cells

#### Matrix Semantics

- 行表示一个 `Skill` 资产
- 列表示一个 `Agent`
- 每一行必须且只能有一个 `source`
- 单元格状态至少包含：
  - `source`
  - `target`
  - `none`
- 点击单元格可直接切换关系

#### Distribution Rule

- `Skill Distribution` 默认动作固定为 `symlink`
- target path 由系统自动计算
- `global skill` 只传播到 `global` 目标
- `project skill` 只传播到同一 `Project` 语境下的目标

#### Required Row Actions

- `Open source`
- `Reveal path`

#### Required States

- `Global` 空态
- `Project` 空态
- source 缺失或不可达状态

#### Prototype Note

- 这里的核心不是“浏览各 Agent 差异”
- 而是“快速表达 `Source Agent -> Target Agents` 的关系”

---

### 9.6 `Prompt` Page

#### Purpose

- 管理 `global prompt` 资产，并通过 `Agent Matrix` 快速配置传播关系

#### Primary Structure

- 无 `Scope switch`
- 直接是 `asset list + Agent Matrix`

#### Row Content

- `Prompt label` 或来源名称
- `Source Agent`
- `source path`
- `Agent Matrix` cells

#### Matrix Semantics

- 与 `Skill` 基本一致
- 每一行必须且只能有一个 `source`
- 传播关系同样由矩阵直接切换

#### Distribution Rule

- `Prompt Distribution` 默认动作固定为 `symlink`
- target path 由系统自动计算

#### Required Row Actions

- `Open source`
- `Reveal path`

#### Prototype Note

- 不要因为只有 `global prompt` 就退回平面列表
- 它仍然是传播关系驱动页面

---

### 9.7 `Session` Page

#### Purpose

- 作为独立内容域，统一搜索、浏览和预览 `Session`

#### Primary Structure

- 顶部 `Source toggle`: `Local / Cloud`
- Search box
- 推荐 `list + preview` 的双栏布局

#### Search Rule

- 必须先选定 `Local` 或 `Cloud`
- 搜索只作用于当前来源
- 支持按文件名和文件内容搜索

#### Global Page Defaults

- 默认来源 = `Cloud`
- `Cloud` 视图展示 WebDAV 汇总后的 `Session`
- `Cloud` 视图是只读聚合视图，不执行跨来源合并、去重或冲突解决
- `Local` 视图聚合当前设备所有已收录 `Project` 的本地 `Session`

#### Result Item Content

- `file name` / title
- `Project`
- 摘要 excerpt
- 更新时间

#### Preview Content

- 基础 metadata
- 内容预览
- `Open file`
- `Open Project`

#### Quick Actions

- 允许少量上下文动作
- 动作粒度保持在 `Project` 级
- 不对单个 `Session file` 做归档/恢复操作
- `Archive now` 与 `Pull now` 必须表达为两个独立方向的动作

#### Project Detail Reuse

- 在 `Project detail` 中复用同一套 `Session view`
- 默认来源 = `Local`
- 仍允许切到 `Cloud`

#### Required States

- `Local empty`
- `Cloud empty`
- `No search results`
- `Cloud unavailable`

#### Prototype Note

- 该页是内容域，不是 `Sync` 页面
- 但允许少量就近动作

---

### 9.8 `Sync` Page

#### Purpose

- 统一承载所有底层任务、模板和执行可观测性

#### Primary Section Order

1. `Skill Distribution`
2. `Prompt Distribution`
3. `Session Backup`
4. `Generic File Distribution/Backup`

#### 9.8.1 `Skill Distribution`

- 展示由 `Skill` 矩阵生成的底层记录
- 记录应标记为 `Managed by Skill`
- 可见、可执行、可观测
- 不在这里编辑 source、target 或 action
- 允许动作包括 `Run now`、查看状态、查看日志、`Open parent asset`

#### 9.8.2 `Prompt Distribution`

- 与 `Skill Distribution` 同理
- 标记为 `Managed by Prompt`

#### 9.8.3 `Session Backup`

- 任务字段建议包括：
  - `Project`
  - `Session Directory`
  - `project key`
  - `Cloud destination`
  - 当前健康状态
- 动作：
  - `Archive now`
  - `Pull now`
- `Archive` 与 `Pull` 是两个独立单向任务方向，不共享同一个可反转任务实例

#### 9.8.4 `Generic File Distribution/Backup`

需要分成两层：

##### A. `Template Library`

- `built-in templates`
- 可包含高价值模板，例如：
  - `SSH Backup`
  - `Zed Config`
  - `Warp Config`
- 目标是快速创建任务，而不是直接执行

##### B. `Task List`

- 展示由模板或手工创建出的具体 `Sync Task`
- 遵守：
  - `single source`
  - `multiple targets`
  - `one-way`

#### Required Sync Actions

- `Create from template`
- `Create custom task`
- `Run now`
- `Open parent asset`（当适用）

#### Prototype Note

- `Sync` 是任务中心，不是资产主入口
- UI 应先按任务类型分组，再展示来源对象

---

### 9.9 `Provider Tray Monitor` (Secondary Surface)

#### Purpose

- 作为 Windows 平台的 companion surface，快速显示 `Provider quota`

#### Scope In Prototype

- 可作为补充小屏或附录表现
- 不是主信息架构原型的第一优先级

## 10. Cross-Screen State Model

### `Provider` State

- `available`
- `no credentials`
- `token expired`
- `request failed`
- `loading`

### `Agent Matrix Cell` State

- `source`
- `target`
- `none`

### `Agent Matrix Row` Rule

- exactly one `source`
- zero or more `target`
- zero or more `none`

### `Session Source` State

- `Local`
- `Cloud`

### `Sync Task` Visibility State

- `managed by Skill`
- `managed by Prompt`
- `manual generic task`
- `template-created task`

## 11. Core User Flows

### Flow 1: Inspect `Provider quota`

1. Open `Provider`
2. View provider cards
3. Refresh or re-check credentials if needed

### Flow 2: Open a `Project` and inspect local context

1. Open `Project`
2. Select a repository
3. Enter `Project detail`
4. Inspect `Skill`, `Session`, `Sync` summary

### Flow 3: Distribute a `Skill`

1. Open `Skill`
2. Choose `Global` or `Project`
3. Locate a row
4. Click `Agent Matrix` cell to set target relation
5. System creates or removes the underlying `Skill Distribution` record

### Flow 4: Toggle `disable-model-invocation`

1. Open `Skill`
2. Locate a row
3. Toggle `disable-model-invocation`
4. System updates only that frontmatter field

### Flow 5: Distribute a `Prompt`

1. Open `Prompt`
2. Locate a row
3. Click target `Agent Matrix` cell
4. System creates or removes the underlying `Prompt Distribution` record

### Flow 6: Search `Session`

1. Open `Session`
2. Choose `Local` or `Cloud`
3. Search by file name or content
4. Inspect results and preview
5. Open original file or open related `Project`

### Flow 7: Archive or pull `Session`

1. From `Project detail` or `Session` page
2. Trigger `Archive now` or `Pull now`
3. Action executes at `Project` granularity

### Flow 8: Create a `Generic File` task from template

1. Open `Sync`
2. Enter `Generic File Distribution/Backup`
3. Pick a template
4. Instantiate a task
5. Run it or inspect it from the task list

## 12. Prototype Deliverables

为保证下一步可直接进入原型设计，至少应产出以下页面原型：

1. `App Shell`
2. `Provider` card view with key states
3. `Project` list
4. `Project detail`
5. `Skill` page (`Global` + `Project`)
6. `Prompt` page
7. `Session` page (`Local` + `Cloud`)
8. `Sync` page with all four type groups
9. `Generic File` template library + task list

## 13. Open Questions For Design Stage

以下问题不再是一级需求矛盾，但会影响原型细化：

- `Agent Matrix` 的视觉编码如何区分 `source / target / none`
- `Session` 页面采用上下布局还是左右双栏布局
- `Project` 列表中的摘要字段密度如何控制
- `Sync` 页面中 template library 与 task list 是同页上下结构还是子页结构
- Windows `Provider Tray Monitor` 在首版原型中是否需要单独呈现
