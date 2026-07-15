# Project custom Skill 传播 module — Design It Twice

日期：2026-07-14

## Shared Constraints

- 允许修改 Rust → Tauri → TypeScript 的读取 interface。
- canonical Skill 与 incoming projection 必须显式区分，但 UI 继续显示独立行和当前 Agent Matrix。
- 读取与写入同时深化；一次用户意图由 `nexus-core` 补偿式原子编排。
- Settings 默认入口 Agent 在执行时由后端解析。
- 仅覆盖 Project custom Skill，遵守 ADR-0003。
- 依赖只有 in-process 与 local-substitutable；不新增公开远程 port 或假想 seam。

## A — Minimal Intent Interface

### Shape

- `list_skills() -> SkillCatalog`
- `scan_skills() -> SkillCatalog`
- `apply_project_custom_skill_intent(intent) -> ApplyResult`
- `SkillRow` 是 `AgentCanonical | ProjectCustomCanonical | ProjectCustomIncoming` 判别联合。
- 写入只有 `SetPropagation` 与 `SetAgentPlacement` 两种 intent。
- mutation 返回完整 catalog，调用者不合并 projection delta。

### Depth

interface 最小，默认 Agent、projection join、placement diff、文件计划、数据库 transaction、逆序补偿与 reconciliation 全在 seam 后。

### Trade-off

leverage 最高，但完整 catalog payload 较大；兼容旧 command 时会短期存在 transport adapter。

## B — Flexible Handle + Intent

### Shape

- canonical row 返回每个 Global/Project target 的 typed handle。
- incoming row返回 placement handle。
- `apply({ handle, intent: propagate | withdraw | setAgentPlacement })`。
- module 内可用 Global / Project 两个真实 destination adapter。

### Depth

调用者完全不拼装 identity；未来增加真实 destination 或 intent 时可扩展 tagged union。

### Trade-off

handle 与私有 destination adapter 有扩展能力，但如果近期只有 Global/Project，opaque handle 或 trait registry 可能过度设计。应优先 typed destination，只有 implementation 差异足够时才保留私有 adapter。

## C — Caller-first Skill Surface

### Shape

- 后端返回 interaction-ready rows。
- 前端增加 `useSkillSurface()`，向 SkillPage / ProjectDetailView / SkillRow 暴露 `rows + submit(intent)`。
- `SkillRow` 多个 callback 收敛为一个 `onIntent`。
- facade 可同时路由 Project custom、Agent-sourced、DMI、Open/Reveal 等所有 Skill 动作。

### Depth

两个页面最简单，前端调用 locality 最强。

### Trade-off

把所有 Skill 动作纳入一个前端 facade 会扩大本任务 scope，并可能形成过宽 interface；Project custom propagation 应有统一 intent，但不必吞并无关的 Agent source relocation、DMI 与文件打开动作。

## D — Core-owned Interaction-ready Surface

### Shape

- `SkillSurface { rows }` 由 core 一次返回。
- Project custom canonical row嵌入 Global + 全部 active Project 的 target state。
- incoming row带 typed/opaque placement reference。
- 保留现有 command 名称，但替换输入/输出 shape；所有 command 委托同一个 core module。

### Depth

projection 与 target read model 完全由 core 拥有；页面不再 join Skills、Projects 与 Settings。

### Trade-off

payload 为 `O(Project custom Skills × active Projects)`，且 Project 状态、排序和默认 Agent 变化都需要 invalidate Skill query。当前本地桌面规模可接受，但 core 不应返回按钮文案、toast 或 CSS/path preview 等 UI 细节。

## Comparison

| Design | Depth | Locality | Seam placement | Main risk |
|---|---|---|---|---|
| A Minimal | 最高：3 个入口隐藏全部编排 | core 最集中 | `SkillService` / 私有 propagation module | 返回完整 catalog；compat adapter |
| B Flexible | 高：handle + intent | core 集中，扩展点明确 | Project custom destination seam | handle/trait 可能提前抽象 |
| C Caller-first | 前端调用 leverage 最高 | 页面最干净 | core + 宽前端 facade | scope 扩大、interface 过宽 |
| D Core-owned | 高：read model 也由 core 拥有 | projection locality 最强 | core read/write surface | eager payload 与跨领域 invalidation |

## Recommended Hybrid

采用 **A + D 为主体，吸收 B 的 typed intent，限制 C 的 facade 范围**：

1. core 返回显式 `SkillRow` 判别联合：`agentCanonical | projectCustomCanonical | projectCustomIncoming`。
2. 所有 row 都有稳定 `rowKey`（只用于渲染）与 canonical `skillId`（用于动作）；不使用 optional identity 或 opaque composite ID。
3. Project custom canonical row由 core 返回 Global + active Projects 的领域 target facts：destination、Project ref、enabled、target agents；不返回文案、toast、CSS 或 path preview。
4. 写入使用一个 typed intent command：
   - `SetTargetEnabled { skillId, destination, enabled }`
   - `SetAgentPlacement { skillId, destination, agent, enabled }`
5. `destination = Global | Project { projectId }` 保持显式、可调试；暂不引入公开 opaque handle 或 plugin registry。
6. mutation 返回权威完整 catalog；React Query 整体替换 Skill cache并 invalidate Project counts。
7. 前端只建立 Project custom propagation 的窄 mutation module；不把 Agent source relocation、DMI、Open/Reveal 全部塞入一个宽 `useSkillSurface`。
8. core 私有 implementation 隐藏 projection join、默认 Agent、placement plan、补偿与 reconciliation。
9. Global 与 Project 若在 implementation 中确实形成两套行为，可保留私有 enum dispatch；不要仅为扩展性建立公开 trait。

## Atomicity Decision

采用 **same-process only**：运行中失败时逆序补偿，仅在补偿实际失败后持久化 reconciliation evidence。不为进程崩溃或强制退出预写 operation journal，也不增加启动/scan 恢复流程；当前没有真实摩擦证明其收益足以覆盖复杂度。
