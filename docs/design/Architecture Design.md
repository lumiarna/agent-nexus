# 产品技术栈和架构设计

> 本文描述 **目标架构（target state）**。结构骨架（workspace、`nexus-core` 领域 crate、`src-tauri` 壳、ports & adapters 边界）已落地；架构评审（见文末「参考」中的 architecture review）确定的 deep module 已同步落到当前代码。实现状态见 [实现状态](#实现状态)。

## 背景

Agent Nexus 是一个跨平台（Mac + Windows）桌面应用，管理多个 AI 编程 Agent 之间的共享资产（Skill、Prompt、Session、Provider）。它依赖深度本地系统集成：文件系统扫描、Symlink/Junction/Copy、WebDAV 同步、SQLite 持久化、系统托盘、定时后台任务。

两个参考实现验证了此栈的可行性：

1. **cc-switch**（`${ROOT}/Sample/cc-switch/`，Tauri 2）— 与 Agent Nexus 高度重叠的产品：多 Agent provider/skill/session 管理 + WebDAV 同步 + SQLite + 托盘。
2. **agent-quota-monitor**（`${ROOT}/Workspace/agent-quota-monitor/`）— 纯 Rust Windows 任务栏 + 托盘 Provider 用量监视器。验证了 Provider quota 抓取、多 provider 并行轮询、飞出窗口（flyout）、图标渲染等能力。

## 决定

采用 **Tauri 2 + Rust 后端 + React 前端**，并把领域逻辑收进一个**不依赖 Tauri 的独立 crate `nexus-core`**，`src-tauri` 只作壳。这样领域代码可独立 `cargo test`，不背 Tauri 运行时。

## 技术栈明细

### 前端

| 选型 | 说明 |
|------|------|
| 框架 | React 18 |
| 构建 | Vite 7（`root: "src-react"`，dev port 3000） |
| 语言 | TypeScript（strict） |
| 样式 | Tailwind CSS 3 + CVA + `cn`；自写 themed 原语（未用 shadcn CLI / Radix） |
| 服务端状态 | TanStack React Query（**已采用**，`lib/query/`） |
| UI 状态 | useState / useReducer（不引入 Zustand 等额外状态库） |
| 图标 | Lucide React |
| 动画 | CSS `@keyframes`（Framer Motion 暂未引入） |
| Toast | Sonner（**已采用**） |
| 拖拽 | @dnd-kit（**已采用**，Sync 任务/分组排序） |
| 表单 | 受控组件（React Hook Form + Zod 暂未引入）；规则校验收进纯 module（taskRules，见核心架构模式 §10） |

#### 前端依赖取舍（现状）

阶段一曾按「先用更轻的等价实现满足原型复刻，待其真正产生价值时再引入」延后若干重量级依赖。随接入真实 IPC，部分已落地：

| 选型 | 现状 |
|------|------|
| 服务端状态 React Query | **已采用**：`lib/query/` 按领域分文件，组件经 hooks 读写，mutation 失效相关 query |
| 拖拽 @dnd-kit | **已采用**：Sync 页任务组与任务排序 |
| Toast Sonner | **已采用** |
| 样式原语 shadcn/Radix | 仍为自写 themed 原语 + 自写 `<Modal>`；需要复杂可访问性组件时再按需引入 Radix |
| 动画 Framer Motion | 仍用 CSS keyframes |
| 表单 RHF + Zod | 仍为受控组件 |

### 后端

| 选型 | 说明 |
|------|------|
| 运行时 | Tauri 2.8+（仅在 `src-tauri` 壳） |
| 领域 crate | `nexus-core`（纯 Rust，无 Tauri 依赖，可独立测试） |
| 语言 | Rust（edition 2021） |
| 数据库 | rusqlite 0.32（Windows 动态链接官方 SQLite，见 ADR-0001；非 Windows 用 bundled） |
| 异步 | Tauri `async_runtime` + reqwest；测试用 Tokio（dev-dependency） |
| HTTP | reqwest 0.12（rustls-tls，关 default features） |
| 序列化 | serde + serde_json |
| 错误处理 | thiserror 枚举 `AppError` + `AppResult`（`nexus-core::error`） |
| WebDAV | 自写传输层（reqwest PROPFIND / MKCOL / PUT，凭据脱敏） |
| 其他 | base64、time、url、uuid(v4)；Windows 额外 `junction` |

### 项目结构

```
agent-nexus/
├── Cargo.toml                       # workspace: members = ["src-tauri", "crates/nexus-core"]
├── src-tauri/                       # Tauri 壳层（薄）
│   ├── src/
│   │   ├── main.rs                  # 入口，调用 agent_nexus_lib::run()
│   │   ├── lib.rs                   # Builder / setup / 状态注入 / 命令注册 / sync 调度线程
│   │   ├── store.rs                 # AppState：装配 nexus-core 各 Service
│   │   └── commands/                # 薄 Tauri 命令：校验输入 → 委托 service → 映射错误
│   │       ├── mod.rs
│   │       ├── app.rs · app_config.rs · agent_capabilities.rs
│   │       ├── projects.rs · project_symlinks.rs
│   │       ├── skills.rs · prompts.rs · sessions.rs
│   │       └── providers.rs · sync.rs
│   ├── Cargo.toml                   # deps: nexus-core(path) · tauri · serde
│   └── tauri.conf.json
│
├── crates/nexus-core/               # 领域后端（无 Tauri 依赖）
│   ├── src/
│   │   ├── lib.rs                   # pub mod database / error / services
│   │   ├── error.rs                 # AppError / AppResult（thiserror）
│   │   ├── database/
│   │   │   ├── mod.rs               # Database：Mutex<Connection>，open / open_in_memory / connection
│   │   │   └── schema.rs            # 迁移：CURRENT_SCHEMA_VERSION + migrate_to_vN（已至 v10）
│   │   └── services/
│   │       ├── mod.rs
│   │       ├── util.rs              # 共享：now_epoch_seconds / required_trimmed / require_agent
│   │       ├── system_open.rs       # 共享：open_path / reveal_path（跨平台分支）
│   │       ├── paths.rs             # 路径解析 / `~` 展开 / 显示规范化
│   │       ├── agent_capabilities.rs# Agent Capability Surface（固定 canonical order）
│   │       ├── app_config.rs        # 设置 KV（config dir、Copilot token）
│   │       ├── projects.rs · project_symlinks.rs
│   │       ├── distribution.rs      # Agent Matrix 传播本体（deep module）
│   │       ├── skills.rs · prompts.rs   # Distribution 的资产 adapter（提供差异）
│   │       ├── sessions.rs
│   │       ├── provider_quota.rs    # adapter trait + 凭据/传输 ports + 纯 derive_snapshot
│   │       ├── placement.rs · symlink.rs   # 文件系统 placement / 链接端口
│   │       ├── cron.rs              # 纯 cron 校验 / 匹配
│   │       ├── webdav.rs            # WebDAV 传输端口
│   │       └── sync.rs · sync/task_lifecycle.rs   # 任务存储 + 校验 + Transfer 编排
│   └── tests/                       # 服务级集成测试（每领域一个文件）
│
├── src-react/                       # 前端（Vite root）
│   ├── App.tsx                      # 状态驱动视图切换（NavContext，无路由库）
│   ├── components/{ui,shell,provider,project,skill,prompt,session,sync,settings}/
│   │   └── sync/taskRules.ts        # 纯规则 module（与后端不变量对齐）
│   ├── lib/
│   │   ├── api/                     # 按领域分文件的 typed invoke 封装
│   │   ├── query/                   # React Query hooks（单一真相源）
│   │   ├── runtime.ts · nav.tsx · tokens.ts · utils.ts
│   └── types/
│
├── prototype/                       # 视觉参考（只读，不参与构建）
├── docs/{design,adr}/
└── CONTEXT.md
```

> 该结构图反映当前代码与目标架构对齐后的落点。

## 核心架构模式

### 1. Workspace 分层：领域 crate 与壳分离

`nexus-core` 承载全部领域逻辑（Service、Database、错误），**不依赖 Tauri**，可独立 `cargo test`；`src-tauri` 只做壳（命令注册、状态注入、窗口/托盘、调度线程）。好处：领域测试不背 Tauri 运行时，编译与测试更快、隔离更干净。

### 2. Command / Service 分离

`src-tauri/src/commands/*` 是薄 Tauri 入口：校验输入 → 委托 `nexus_core::services` → 映射错误。业务逻辑不出现在 command 中。

```rust
#[tauri::command]
pub async fn run_task(state: State<'_, AppState>, id: String) -> AppResult<Task> {
    state.sync.run_task(id).await
}
```

`AppState`（`store.rs`）持有所有 Service，并通过 `Arc<Database>` 共享同一连接句柄。

### 3. 共享基础设施收口（拒绝重复定义）

跨 service 复用的基础设施只定义一次，不在每个领域文件里各写一份：

- `services/util.rs`：`now_epoch_seconds`、`required_trimmed`、`require_agent`
- `services/system_open.rs`：`open_path`、`reveal_path`（跨平台分支）
- `services/paths.rs`：路径解析、`~` 展开、显示规范化

原则：同一函数若在多个 service 出现逐字副本，应上提到共享 module；领域差异才留在领域文件。

### 4. Distribution module — deep 的 Agent Matrix

`Skill` 与 `Prompt` 的传播（`Agent Matrix`）共享同一套不变量，收进一个 deep `services/distribution.rs`：

- 「每个资产恰好一个 `source`」「`source` ≠ `target`」
- `role`（source / target / none）派生与 `cells` 装配
- `Placement` 建立 + 失败回滚、扫描归并（upsert + 替换 distribution 行 + 清理失效）
- `symlink_points_to`（判断某 agent 当前是否为 target）

`Skill` / `Prompt` 作为 **adapter** 只提供差异：`Scope`（global/project）、Placement 原语（目录 link vs 文件 symlink）、`target_path` 计算规则、表名。一处不变量，两资产复用；新增可传播资产近零成本。

### 5. Provider quota — adapter trait + 内层 ports

外层 `ProviderQuotaAdapter` trait 按 `provider_id` 选择具体 provider（Claude Code / CodeX / Copilot）。每个 provider 的副作用退到内层两个 port 之后：

- **CredentialSource**：读取凭据（文件 / macOS Keychain / 环境）
- **UsageTransport**：抓取用量（reqwest）

状态派生是纯函数 `derive_snapshot(creds, usage) -> ProviderQuotaSnapshot`，四种 status（available / expired / failed / nocreds）全分支可通过内存 fake adapter 测试，`quota()` 接口不再触网。

### 6. Sync — 任务存储 + 纯 cron + Transfer seam

- **任务存储 / 校验**：`sync.rs`、`sync/task_lifecycle.rs`（Task / Task Group CRUD、`prepare_task` 不变量、`derive_direction`）。
- **cron**：纯 `services/cron.rs`（校验 + 是否命中某分钟），可直测。
- **Transfer seam**：Local→Cloud 传输抽象为 `Transfer` 端口，`webdav` adapter 用于生产，`RecordingTransfer` 用于测试；`run_task` 收薄为编排（状态记录、调度门控、回滚）。
- **编排位置**：当前跨领域复杂度只涉及 Sync 自身、WebDAV 与 Placement，`sync/task_lifecycle.rs` 仍足够 deep；等执行器开始组合 Project / Session / Provider 等多个领域时，再抽独立 `orchestration/` 目录。

`Sync Task` 永远单向、单 source → 单 target；`Direction` 由 `Location Type` 派生（Local+Local=Distribution，Local+Cloud=Push，Cloud+Local=Pull，Cloud+Cloud 非法）。

### 7. 轻量 Ports & Adapters 边界

只在外部副作用边界使用轻量端口，不全面六边形化：

| 边界 | 模块 | 生产 adapter | 测试 adapter |
|------|------|------|------|
| 文件系统 link / placement | `symlink.rs` · `placement.rs` | OS symlink / junction | tempdir |
| WebDAV 传输 | `webdav.rs` | reqwest | —（经 Transfer） |
| Sync 传输 | `Transfer` | `webdav` | `RecordingTransfer` |
| Provider 凭据 | `CredentialSource` | 文件 / Keychain | fake |
| Provider 用量 | `UsageTransport` | reqwest | fake |
| 系统打开 / 揭示 | `system_open.rs` | `open` / `explorer` / `xdg-open` | — |

**`Database` 不抽 port**：rusqlite `Mutex<Connection>` 作为本地持久化基础设施直接被 Service 使用，避免为尚未变化的边界提前造 trait。

### 8. 前端 API 层

每个领域一个 typed namespace（`lib/api/*`），组件不直接调 `invoke()`。

```typescript
export const syncApi = {
  listTaskGroups: () => invoke<TaskGroup[]>("list_task_groups"),
  runTask: (id: string) => invoke<Task>("run_task", { id }),
};
```

### 9. React Query 单一真相源

所有来自后端的数据走 React Query；**组件不再用 `useState` 维护服务端状态的本地镜像**。Mutation 通过 `invalidateQueries` 或 `setQueryData` 更新缓存，避免「本地镜像 vs 缓存」两份真相分叉。

### 10. 前端纯规则 module（taskRules）

`Sync Task` 的 UX 校验（action↔location 兼容、Junction 仅 Windows、schedule 仅 Copy、direction 派生、单 source→单 target）收进纯 `sync/taskRules.ts`，create / add 两个表单共用一个 interface、可单测。后端 `prepare_task` 仍是事务级真相源（SSOT），前端规则与之对齐。

### 11. 状态驱动视图切换（无路由库）

桌面应用不需要 URL 路由。`View` 联合 + `NavContext` 即可：

```typescript
type View = "provider" | "project" | "skill" | "prompt" | "session" | "sync" | "settings";
```

### 12. Sync 后台调度

`lib.rs` 启动一个后台线程，每分钟对齐边界轮询 `sync.run_due_scheduled_tasks(now)`，驱动 CRON 调度的 Copy 任务。

### 13. Manifest 同步协议（参考 cc-switch）

配置/资产多设备同步沿用 manifest 快照协议（先传 artifacts 再传 manifest，下载校验 SHA-256）。多设备双向通过两个显式反向 `Sync Task`，不在单 Task 内回流。

## 数据库设计

详见 [数据库 Schema 设计](<./Database Schema.md>)。要点：rusqlite 直写 SQL（无 ORM）；`database/schema.rs` 按 `CURRENT_SCHEMA_VERSION` 顺序执行 `migrate_to_vN`（现已演进至 v10），每次迁移独立事务；表命名对齐 `CONTEXT.md` glossary。

## 实现状态

| 模块 / 模式 | 状态 |
|------|------|
| workspace 分层（`nexus-core` + `src-tauri` 壳） | ✅ 已落地 |
| Command / Service 分离 | ✅ 已落地 |
| 共享基础设施收口（`util.rs` / `system_open.rs` / `paths.rs`） | ✅ 已落地 |
| ports & adapters：`symlink` / `placement` / `webdav` / `system_open` | ✅ 已落地 |
| Provider quota adapter trait（按 provider 选择） | ✅ 已落地 |
| `sync` 任务存储 / 校验 / CRON 调度线程 | ✅ 已落地 |
| 前端 `lib/api` + React Query | ✅ 已落地 |
| 状态驱动视图切换（NavContext） | ✅ 已落地 |
| Distribution module（Skill/Prompt 收口为 adapter） | ✅ 已落地 |
| Provider quota 内层 CredentialSource / UsageTransport ports + 纯 derive | ✅ 已落地 |
| 纯 `cron.rs` + `Transfer` seam | ✅ 已落地 |
| 前端 `taskRules` 纯规则 module | ✅ 已落地 |
| React Query 单一真相源（移除 SyncPage 本地镜像） | ✅ 已落地 |

## 后果

### 正面

- 领域 crate `nexus-core` 可独立测试，不背 Tauri；服务级测试覆盖各领域。
- 公共 helper 收口减少重复定义，规则改动只动一处。
- deep module（Distribution、provider quota 派生、cron、Transfer）提升 locality 与可测性：interface 即测试面，I/O 退到 seam 之后。
- 跨平台一套代码（Mac + Windows + Linux），安装包 ~4-10 MB（vs Electron ~150 MB）。
- React Query 单一真相源消除本地镜像与缓存分叉。

### 负面

- Rust 学习曲线；调试跨两种语言（TS 前端 + Rust 后端）。
- rusqlite 需 `Mutex<Connection>` 包装（无原生 async SQLite）。
- 轻量端口 / adapter 增加少量 trait 代码。

### 中性

- 前端技术（React + Vite + Tailwind）与壳选型无关。
- `Database` 在 MVP 不抽 port；未来仅在需要替换持久化或显著提升测试隔离时再评估。

## 被否决的替代方案

| 替代 | 否决原因 |
|------|----------|
| Electron + Node.js | 150 MB 包体；cc-switch 证明 Tauri 对此类问题可行 |
| AppKit / SwiftUI | 仅 macOS，违反跨平台需求 |
| Flutter | AI 辅助生产力低；native 交互痛点 |
| JSON 文件（不用 SQLite） | 一旦需要搜索/聚合就必须迁移 |
| Zustand/Jotai | React Query 已覆盖服务端状态，多一层状态库无必要 |
| 把领域逻辑放进 `src-tauri` | 领域测试会被 Tauri 运行时拖累；故拆出 `nexus-core` crate |
| `Database` 抽象为 port | MVP 内持久化边界不变，提前造 trait/adapter 无收益 |

## 参考

| 参考 | 路径 | 用途 |
|------|------|------|
| cc-switch | `${ROOT}/Sample/cc-switch/` | 整体架构、WebDAV sync、SQLite DAO、service 分层、前端模式、Provider quota 刷新模型 |
| cc-switch sync protocol | `src-tauri/src/services/sync_protocol.rs` | manifest 同步协议设计 |
| cc-switch schema | `src-tauri/src/database/schema.rs` | 迁移模式、表设计参考 |
| agent-quota-monitor | `${ROOT}/Workspace/agent-quota-monitor/` | Windows 托盘、flyout、图标渲染 |
| Agent Nexus prototype | `prototype/*.dc.html` | 视觉和交互参考 |
| Agent Nexus 领域模型 | `CONTEXT.md` | 实体定义与边界 |
| 架构评审（deepening） | 临时 HTML 报告（`improve-codebase-architecture`） | Distribution / quota ports / cron+Transfer / taskRules / RQ 单一真相源等目标态来源 |
