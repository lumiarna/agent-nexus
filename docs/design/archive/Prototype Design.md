# 原型设计

## 1. 产品概述

`Agent Nexus` 是一个面向个人 AI 编程环境的桌面应用，作为资产中枢运作。它不把每个 Agent 当作孤岛来管理，而是把 `Provider / Project / Skill / Prompt / Session` 视为一组可复用、可分发、可观测的共享资产。

## 2. 产品目标

### 主要目标

- 在一个桌面应用中统一查看和管理 AI 编程环境资产
- 不必按单个 Agent 分别维护 Skill、Prompt、Session 和同步关系
- 以"共享资产"视角完成发现、传播、归档、搜索与观察

### 用户价值

- 看清 Provider quota
- 看清 Project 与共享资产的关系
- 快速把 Skill / Prompt 传播给其他 Agent
- 搜索 Session，并在 Local / Cloud 之间切换
- 通过 Sync 管理底层任务

## 3. 目标用户

使用多个 AI coding agents 的个人开发者，同时维护多个 Git 仓库，有跨项目复用 Skill / Prompt 的需求，有跨设备检索与归档 Session 的需求。

## 4. MVP 范围

### 包含

- Provider 全局 quota 可见性 + 配置（连接参数、显示偏好）
- Project 通过 Git Base Folder 自动发现 + 手动收录
- Skill 的 global / project 管理与 Agent Matrix 传播
- Prompt 的 global 管理与 Agent Matrix 传播
- Session 的 Local / Cloud 双来源视图与搜索
- Sync 任务中心（Task Group 模型）
- Generic File Distribution/Backup（通过 Task Group + 模板）
- Settings 页面（WebDAV、Git Base Folders、任务栏指标、Agent config roots）

### 不包含

- Provider 安装、登录、账号生命周期管理
- 双向同步、多源合并同步
- Project 级别的 quota 归因
- Project 级别的 Prompt
- 通用内容编辑器
- 除 `disable-model-invocation` 外的 frontmatter 编辑
- Session 多目录来源
- Template 对实例的回流控制
- 跨 Local / Cloud 的混合 Session 搜索
- MVP 内的 CRON 自动调度运行时（UI 展示但不实际执行）

## 5. 平台与 UI 默认值

- 平台：Windows + macOS
- 主题：Light / Dark（默认 Light）
- 语言：英文（MVP 不做多语言）
- 路径显示：规范化为 `/`

## 6. 核心产品原则

1. **共享资产优先** — 优先将对象建模为共享资产，而非 Agent 各自为政的私有配置。
2. **单源多目标** — 所有 Sync 遵守 single source → multiple targets 单向规则。
3. **强默认、显式覆写** — 能自动推导的优先自动推导，例外用显式 override。
4. **上下文页 + 任务中心** — 上下文页负责就近操作，Sync 页负责统一观测。
5. **拒绝启发式魔法** — 只基于系统已知关系做归并，不依赖名称/内容相似性猜测。
6. **资产类型不全部可分发** — Skill/Prompt 可分发；Session 可搜索可归档；Project/Provider 是上下文/观测资源。
7. **Backup 与 Pull 是独立单向任务** — 不通过反转既有 Backup 来模拟双向同步。

## 7. 信息架构

一级导航顺序固定：

1. `Provider`
2. `Project`
3. `Skill`
4. `Prompt`
5. `Session`
6. `Sync`

另有 `Settings` 页面，从标题栏齿轮图标进入。

## 8. Agent 定义

规范顺序（用于传播矩阵和配置根展示）：

| Agent | 缩写 | 配置目录 | 备注 |
|-------|------|---------|------|
| Agents | AG | `~/.agents` | 通用默认放置目录 |
| Claude Code | CC | `~/.claude` | |
| CodeX | CX | `~/.codex` | |
| Copilot | CP | `~/.github` | MVP 中作为完整 Agent 参与 Skill/Prompt 传播 |
| OpenCode | OC | `~/.config/opencode` | |

所有涉及 Agent 排列的 UI 严格按此顺序。

## 9. 页面规格

---

### 9.1 App Shell

- 标题栏：窗口控件 + 居中 "Agent Nexus" 品牌标识 + 搜索药丸 (`⌘K`) + 设置齿轮图标
- Tab 导航栏：Provider / Project / Skill / Prompt / Session / Sync
- 激活态 tab 视觉突出（药丸样式）
- 桌面生产力工具密度，不是营销着陆页

---

### 9.2 Provider 页面

**目的：** 统一查看每个 Provider 的 quota、reset 与 credential 状态。

**布局：** 卡片网格（auto-fill，最小 300px）。卡片支持拖拽排序。

**卡片内容：**
- Provider 名称 + Agent 徽章（若同时是 Agent）
- 状态徽章（Available / Token expired / Request failed / No credentials / Checking…）
- 主要 quota 百分比（大字号）
- 各窗口 quota 进度条 + 重置信息
- Credential 来源（等宽字体）
- 快捷操作：Refresh / Re-check / Retry + 配置齿轮

**配置弹窗（每个 Provider）：**
- 连接参数区（如 OpenCode Go 的 Workspace ID、Auth Cookie）
- 显示偏好区：
  - 是否在 Provider 页面显示卡片（开关）
  - 是否在 Windows 任务栏显示（开关，独立于卡片可见性）
- 说明：任务栏指标（used/remaining）是全局设置 → 链接到 Settings

**隐藏卡片：** 在页面底部以虚线边框区域展示，每个可点击 "show" 恢复。

**状态集：** available / no credentials / token expired / request failed / loading

---

### 9.3 Project 页面（列表）

**目的：** 展示所有已收录的 Git 仓库，作为工作区上下文入口。

**表格列：** 拖拽手柄 | Project（名称 + session 目录） | 仓库路径 | 资产计数（Skill/Session/Sync） | 溢出菜单

**状态徽章：** Active / Stale（仓库路径已不可达）

**操作：**
- "Add Git Base Folder" — 打开扫描弹窗（选择文件夹、扫描、勾选新仓库录入）
- "Add Project" — 录入单个仓库根目录

**Project Key：** 始终为文件夹名称。UI 不提供编辑入口。Add 弹窗中仅作为信息展示。

**隐藏项目：** 以虚线区域展示，可 "unhide"。

**溢出菜单：** Hide / Delete…（含确认弹窗，展示级联数据影响）

---

### 9.4 Project 详情页

**目的：** 在单个 Project 上聚合本地上下文、资产关系与 project-bound 操作。

**头部：** Project 名称、仓库路径、key（文件夹名）、session 目录。

**快捷操作：** Archive now / Pull now / Open in Sync

**区块：**

1. **Skill** — 与全局 Skill 列表相同的列（名称+源 Agent 徽章、传播图标、disable-invoke 开关、open/reveal）。仅展示该 project 范围的 skill。
2. **Session** — Local/Cloud 切换，session 列表（标题/摘要/更新时间）。链接到 Session 页面。
3. **Sync 摘要** — Skill Distribution 状态、Session Backup 状态、Generic File 状态。

**不包含 Prompt 区块。**

---

### 9.5 Skill 页面

**目的：** 管理 Skill 资产，通过 Agent Matrix 配置传播关系。

**工具栏：**
- 范围切换：Global / Project
- 搜索输入框（按名称或描述搜索）
- 图例：source（实心背景）/ target（染色背景）/ none（虚线边框）

**当选择 Project 范围时：** 显示 Project 选择器芯片（All + 每个活跃 project）

**表格列：** Skill（名称 + 源 Agent 徽章 + 描述） | Distribution（Agent 图标） | Disable invoke（开关） | Source file（Open source / Reveal path 链接）

**Agent Matrix（紧凑图标形式）：**
- 每个 Agent 显示为小方块 + 2 字母缩写
- Source：实心背景，Agent 色，白色文字
- Target：染色背景，Agent 色文字，实线边框
- None：透明，虚线边框，灰色文字
- 点击在 target/none 之间切换（source 固定不可改）

**规则：**
- 每行恰好一个 source
- Global skill 只传播到 global 目标
- Project skill 只传播到同一 project 下的目标
- 传播动作 = symlink（固定）
- 目标路径由系统自动计算

**状态：** Global 空态、Project 空态、搜索无结果

---

### 9.6 Prompt 页面

**目的：** 管理全局 Prompt 资产，通过 Agent Matrix 配置传播关系。

**无范围切换** — 始终是 global。

**表格列：** Prompt（名称 + 源 Agent 徽章 + 源路径等宽字体展示） | Distribution（Agent 图标） | Source file（Open source / Reveal path）

**Agent Matrix 语义与 Skill 一致**（紧凑图标，点击切换 target/none）。

**传播动作：** symlink（固定），目标路径自动计算。

---

### 9.7 Session 页面

**目的：** 作为独立内容域，统一搜索、浏览和预览 Session。

**布局：** 双栏 — 左侧列表（380px）+ 右侧预览面板。

**控件：**
- 来源切换：Local / Cloud（默认 = Cloud）
- 搜索输入框（仅在当前来源内搜索）
- Project 筛选芯片

**列表项：** 标题（等宽）/ Project 标签 / 摘要 / 更新时间。选中项以 accent 色左边框标记。

**预览面板：**
- 标题、元数据（Project、Updated、Size、Source）
- 文件路径
- 操作：Open file / Open Project（→ Project 详情页） / Archive now / Pull now
- 内容预览（等宽字体，保留格式）

**Cloud 显示：** 仅显示 "Cloud"，不加 "(WebDAV)" 后缀。WebDAV 配置仅在 Settings 出现。

**Open Project：** 跳转到具体 Project 详情页，不是 Project 列表。

**状态：** Local empty / Cloud empty / No search results / Cloud unavailable（含 retry 按钮）

---

### 9.8 Sync 页面

**目的：** 任务工作台，承载所有同步任务与可观测性。

**主体区域："Your Task Groups"**

- Task Group 之间支持拖拽排序
- 每个 Group 是一张卡片：
  - 拖拽手柄 + 组名 + 任务数 + "Run group" / "Add task" 按钮
  - 列头：(拖拽) | Type | Source | Target | Schedule | Status
  - 组内 Task 支持拖拽排序
  - 每个 Task：方向徽章（Distribution/Backup/Restore-Pull）+ 动作标签、源路径、目标、schedule 芯片（点击编辑）、上次运行、状态 + Run 链接

**Schedule（每个 Task 级别）：**
- Manual 或 CRON
- CRON 有表达式输入 + 预设（Hourly / Daily 05:00 / Weekly Sun 03:00）
- 已有 Task 的 schedule 可通过弹窗单独编辑

**"Create custom task" 弹窗：**
- 模板选择芯片（Blank / Machine Backup / Warp Config / Dotfiles）— 模板预填整个 Task Group 的多个 Task
- Task Group 名称输入
- Task 列表（可增删）：
  - 方向选择器：Distribution / Backup / Restore-Pull
  - 动作选择器：symlink / copy
  - Source 输入（单个）
  - Targets 列表（一个或多个，可增删）
  - Schedule：Manual / CRON（含表达式 + 预设）

**系统管理记录（页面底部，默认折叠）：**
- Skill Distribution（由 Skill 矩阵管理）
- Prompt Distribution（由 Prompt 矩阵管理）
- Session Backup（由 Session 管理）
- 每个区块可展开，显示 asset/relation/target-path/status
- 只读 — 不可在此编辑 source/target/action

**关键设计决策：**
- 模板隐藏在 "Create custom task" 流程内，不单独展示在页面上
- 没有实际创建的 Task 不出现在页面
- Backup/Distribution/Restore-Pull 方向绑定在 Task 级别，非 Task Group 级别
- CRON 定时器绑定在 Task 级别

---

### 9.9 Settings 页面

**目的：** 全局配置界面。

**区块：**

1. **WebDAV** — 端点 URL、用户名、密码/App token、测试连接、保存。状态指示器（Connected / Not tested / Testing…）。说明：在应用其他位置统一显示为 "Cloud"。
2. **Git Base Folders** — 扫描目录列表 + 删除操作 + 添加文件夹按钮。
3. **Windows 任务栏** — Quota 指标切换（Used / Remaining）。默认 = Remaining。全局统一应用。
4. **Agent config roots** — 只读展示每个 Agent 的配置目录、skills 目录、prompt 文件。顺序：Agents / Claude Code / CodeX / Copilot / OpenCode。

---

## 10. 跨页面状态模型

### Provider 状态
`available` | `no credentials` | `token expired` | `request failed` | `loading`

### Agent Matrix 单元格状态
`source` | `target` | `none`

### Agent Matrix 行规则
恰好一个 `source`，零个或多个 `target`，零个或多个 `none`。

### Session 来源状态
`Local` | `Cloud`

### Sync Task 可见性状态
`managed by Skill` | `managed by Prompt` | `managed by Session` | `手动/模板创建的任务`

## 11. 核心用户流程

1. **查看 Provider quota** → Provider 页面 → 查看卡片 → 刷新/重新检查
2. **打开 Project 上下文** → Project 列表 → 选择 → Project 详情 → 查看 Skill/Session/Sync
3. **传播一个 Skill** → Skill 页面 → 选择范围 → 定位行 → 点击 Agent 图标切换 target
4. **切换 disable-model-invocation** → Skill 页面 → 切换开关
5. **传播一个 Prompt** → Prompt 页面 → 定位行 → 点击 Agent 图标
6. **搜索 Session** → Session 页面 → 选择 Local/Cloud → 搜索 → 预览 → 打开文件/项目
7. **归档/拉取 Session** → Project 详情或 Session 页面 → Archive now / Pull now（Project 粒度）
8. **创建 Generic File 任务** → Sync 页面 → Create custom task → 选模板 → 填写 → 创建

## 12. 关键反馈决策汇总

| 决策点 | 结论 |
|--------|------|
| Provider 卡片排序 | 支持拖拽 |
| Provider 配置 | 分两类：连接参数 + 显示偏好 |
| Project Key | 始终为文件夹名，不可编辑 |
| Copilot | MVP 中作为完整 Agent 参与传播 |
| Skill 行高 | 缩减：不显示源路径，Open/Reveal 放到列里 |
| Agent Matrix 视觉 | 紧凑 Agent 图标（非每 Agent 一列） |
| Prompt 源路径 | 保留展示 |
| WebDAV 配置 | 仅在 Settings 页面出现；其他地方显示 "Cloud" |
| Sync 布局 | 系统记录在底部，默认折叠 |
| 模板 | 隐藏在 "Create custom task" 弹窗内 |
| Task Group | 支持多个 Task；模板配置的也是完整 Task Group |
| 方向/定时器 | 绑定在 Task 级别 |
| Task/Group 排序 | 均支持拖拽 |
| Session 页面默认来源 | Cloud；不加 "(WebDAV)" 后缀 |
| Session → Project 链接 | 跳转到 Project 详情页 |
| `New in MVP` 标签 | 已删除 |
