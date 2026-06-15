# Agent Nexus

`Agent Nexus` 是一个面向个人 AI 编程环境的资产中枢：它不把每个 Agent 当成彼此独立的孤岛来管理，而是把 `Provider / Project / Skill / Prompt / Session` 视为一组可复用、可分发、可观测的共享资产。

## Product

**Agent Nexus**:
一个围绕 `shared assets`、`Project` 和 `Sync` 组织的本地工作台。它把多个 agent 当作资产的消费端，而不是把产品本身建模成按 agent 切开的配置面板。
_Avoid_: Multi-agent config app, agent launcher, sync client

**Asset**:
在本项目中，`Asset` 指被系统识别、展示、关联、传播或观测的领域对象，而不是任意文件副本。当前核心 `Asset` 包括 `Skill`、`Prompt`、`Session`、`Project` 和 `Provider`；但并非所有 `Asset` 都可传播。
_Avoid_: File, record, resource

**Distributable Asset**:
一种可以从 `canonical source` 建立 `Distribution` 并落到其他 agent 消费端的 `Asset`。MVP 中主要包括 `Skill` 和 `Prompt`，不包括 `Project`、`Provider` 或 `Session`。
_Avoid_: Every asset is distributable, backup item, global resource

**Archivable Content**:
一种以搜索、归档和恢复为核心价值的内容型 `Asset`。MVP 中典型对象是 `Session`；它可以进入 `Backup` / `Restore/Pull`，但不进入 `Agent Matrix` 传播模型。
_Avoid_: Distributable skill, prompt placement, generic file task

**Shared Asset**:
一个应在多个 agent 或多个工作区之间复用的 `Asset`。它强调“共享关系”而不是“归属于单一 agent 的私有配置”。
_Avoid_: Per-agent config, local-only file

## Workspace

**Project**:
一个被 Agent Nexus 收录的 Git repository root。它是 `Session`、`project Skill` 和 project-bound `Sync` 任务的上下文边界，而不是任意目录。
_Avoid_: Folder, workspace path, arbitrary directory

**Git Base Folder**:
用于自动发现 `Project` 的扫描根目录。它不是 `Project` 本身，只是 `Project discovery` 的输入范围。
_Avoid_: Project, workspace

**Project Key**:
用于跨设备归并同一 `Project` 的稳定身份键。它默认从 canonical Git remote 派生，可手工覆写，并用于 `Session` 的 WebDAV 归档与聚合。无 remote 时可退回项目目录名；存在多个 remote 且无法确定 canonical remote 时，需要用户确认或覆写，不自动退回目录名。
_Avoid_: Local path, folder name, remote guess

## Assets

**Skill**:
一种可被 agent 消费的共享能力资产。`Skill` 同时支持 `global` 与 `project` 两种 `Scope`，并以 `canonical source` 作为资产身份。
_Avoid_: Skill copy, skill target, generated placement

**Prompt**:
一种面向 agent 的全局提示资产。MVP 中 `Prompt` 只覆盖 `global prompt file`，不纳入 `project-level prompt`。
_Avoid_: Project prompt, prompt copy

**Session**:
一种可搜索、可归档的会话内容资产。它既有 `Local` 视图，也有 `Cloud` 视图，但归档与恢复的任务粒度保持在 `Project` 级。
_Avoid_: Chat log only, backup artifact only

**Provider**:
一个提供 quota 信息与 credential source 的外部服务身份。`Provider` 在本项目中是全局资源，不做 project-level quota 归因。
_Avoid_: Project provider, account manager

## Identity

**Canonical Source**:
一个 `Distributable Asset` 的权威来源位置。只有 `canonical source` 才算该可传播资产本体，所有 `symlink` 或 `copy` 产生的目标都只是 `placement` 或 `target`。
_Avoid_: Copy target, mirrored file, duplicate asset

**Placement**:
一个已知由系统管理、并与某个 `canonical source` 相关联的落点。它表示资产被放到了哪里，而不是产生了一个新资产。
_Avoid_: New asset, cloned asset

**Scope**:
`Asset` 的适用范围。当前只用于 `Skill`，取值为 `global` 或 `project`。
_Avoid_: Type, category

## Distribution

**Distribution**:
把一个 `Distributable Asset` 从 `canonical source` 传播到其他消费端的单向关系。它强调关系建立与目标落点，而不是双向同步。
_Avoid_: Two-way sync, replication mesh

**Agent Matrix**:
`Skill` 和 `Prompt` 页面中的关系矩阵视图。行表示资产，列表示 agent，单元格表示 `source / target / none` 这类传播关系；每一行必须且只能有一个 `source`。
_Avoid_: Agent tree, config table, task grid

**Source Agent**:
在 `Agent Matrix` 中拥有该资产 `canonical source` 的 agent。它表示关系起点，而不是运行时执行者。
_Avoid_: Owner, primary target

**Target Agent**:
在 `Agent Matrix` 中接收资产传播关系的 agent。对 `Skill` 和 `Prompt`，其目标路径由系统按 agent 与上下文自动计算。
_Avoid_: Secondary source, duplicate source

## Sync

**Sync**:
Agent Nexus 的任务与预设工作域。它统一承载 `Distribution`、`Backup` 和 `Restore/Pull` 的底层执行与可观测性，但不承担资产主视图。`Backup` 与 `Restore/Pull` 是两个显式的单向任务方向，不合并成一个双向任务。
_Avoid_: Asset page, file manager, two-way sync engine

**Sync Task**:
一个具体的单向执行任务，遵守 `single source -> multiple targets` 规则。它是运行对象，不是模板，也不是资产本体；如果需要从 WebDAV 拉回本地，应创建独立的 `Restore/Pull` 任务，而不是反转既有 `Backup` 任务。
_Avoid_: Template, relationship only, background daemon

**Template**:
用于快速创建 `Generic File Distribution/Backup` 任务的预设定义。它是创建加速器，实例化后不回流控制既有任务。
_Avoid_: Task, live config, inherited instance

**Instance**:
由 `Template` 或手工操作创建出的具体 `Sync Task`。它创建后独立存在，不自动跟随模板变更。
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
围绕 `Session` 归档建立的 `Sync` 类型。它负责把 `Project` 的本地 `Session` 目录归档到 WebDAV，而不是管理会话内容本身。
_Avoid_: Session viewer, session search

**Backup**:
从本地权威源复制到远端或其他目标的单向操作。它不要求目标与源严格镜像，也不默认传播删除。
_Avoid_: Mirror sync, two-way sync

**Restore/Pull**:
从 WebDAV 等远端归档源显式拉回到本地的受限单向操作。它用于恢复或落地配置，不等于双向同步。
_Avoid_: Reverse sync, cloud source by default

## Session Views

**Local Session**:
来自当前设备、已收录 `Project` 的本地会话集合。它是 `Session` 页在 `Local` 来源下的真相源。
_Avoid_: Cloud cache, merged archive

**Cloud Session**:
来自 WebDAV 汇总归档的会话集合。它代表跨设备聚合后的会话视图，是 `Session` 页在 `Cloud` 来源下的真相源。这里的聚合是只读视图聚合，不表示多源合并同步、冲突解决或远端默认成为通用 source。
_Avoid_: Local session, mixed source view

**Session Directory**:
一个 `Project` 的本地会话目录。默认模板是 `${project_dir}/__sessions`，但可被 project-level override 覆写；MVP 中每个 `Project` 只允许一个 `Session Directory`。
_Avoid_: Multi-root session source, fixed hardcoded path

## Boundaries

**Credential Visibility**:
对 `Provider` 凭据来源与可用性的展示能力。它只负责发现与诊断，不接管第三方身份生命周期。
_Avoid_: Login flow, account management, token writer

**Quick Action**:
在 `Session` 或 `Provider` 等上下文页中就近提供的轻量动作。它缩短操作路径，但不替代 `Sync` 的完整任务定义能力。
_Avoid_: Full configuration surface, task editor

**Global Resource**:
不应被 project-level 归因或切分的资源。当前 `Provider quota` 是典型的 `Global Resource`。
_Avoid_: Project quota, per-repo quota
