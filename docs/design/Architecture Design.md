# 产品技术栈和架构设计

## 背景

Agent Nexus 是一个跨平台（Mac + Windows）桌面应用，管理多个 AI 编程 Agent 之间的共享资产（Skill、Prompt、Session、Provider）。它依赖深度本地系统集成：文件系统扫描、Symlink/Copy、WebDAV 同步、SQLite 持久化、系统托盘、定时后台任务。

两个参考实现验证了此栈的可行性：

1. **cc-switch**（`D:\Sample\cc-switch\`，Tauri 2 v3.16.x）— 与 Agent Nexus 高度重叠的产品：多 Agent provider/skill/session 管理 + WebDAV 同步 + SQLite + 托盘。已在 Windows/macOS/Linux 上线。
2. **agent-quota-monitor**（`D:\Workspace\agent-quota-monitor\`）— 纯 Rust Windows 任务栏 + 托盘 Provider 用量监视器。验证了 Provider quota 抓取、多 provider 并行轮询、飞出窗口（flyout）、图标渲染等能力。

## 决定

采用 **Tauri 2 + Rust 后端 + React 前端**。

## 技术栈明细

### 前端

| 选型 | 说明 |
|------|------|
| 框架 | React 18 |
| 构建 | Vite 7（`root: "src-react"`，dev port 3000） |
| 语言 | TypeScript（strict） |
| 样式 | Tailwind CSS 3 + shadcn/ui（Radix + CVA） |
| 服务端状态 | TanStack React Query |
| UI 状态 | useState / useReducer（不引入 Zustand 等额外状态库） |
| 图标 | Lucide React |
| 动画 | Framer Motion |
| Toast | Sonner |
| 拖拽 | @dnd-kit |
| 表单 | React Hook Form + Zod |

### 后端

| 选型 | 说明 |
|------|------|
| 运行时 | Tauri 2.8+ |
| 语言 | Rust（edition 2021） |
| 数据库 | rusqlite（bundled，含 FTS5） |
| 异步 | Tokio（multi-thread） |
| HTTP | reqwest（rustls-tls） |
| 序列化 | serde + serde_json |
| 错误处理 | thiserror 枚举 (`AppError`) |
| WebDAV | 自写传输层（reqwest PUT/GET/MKCOL） |

### 项目结构

```
agent-nexus/
├── src-react/                    # 前端（Vite root）
│   ├── main.tsx
│   ├── App.tsx                   # 视图切换器（状态驱动，无路由库）
│   ├── components/
│   │   ├── ui/                   # shadcn/ui 原语
│   │   ├── project/
│   │   ├── skill/
│   │   ├── session/
│   │   ├── provider/
│   │   ├── prompt/
│   │   └── sync/
│   ├── lib/
│   │   ├── api/                  # 按领域分文件的 typed invoke 封装
│   │   ├── query/                # React Query hooks
│   │   └── utils.ts
│   ├── hooks/
│   ├── types/
│   └── index.html
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs                # 插件/命令注册
│   │   ├── commands/             # 薄命令层（校验 → service → map_err）
│   │   │   ├── project.rs
│   │   │   ├── skill.rs
│   │   │   ├── prompt.rs
│   │   │   ├── session.rs
│   │   │   ├── provider.rs
│   │   │   ├── sync.rs
│   │   │   └── settings.rs
│   │   ├── services/             # 业务逻辑层
│   │   │   ├── project.rs       # Git 仓库扫描、路径续认
│   │   │   ├── skill.rs         # SSOT + distribution + agent matrix
│   │   │   ├── session.rs       # 目录扫描、索引
│   │   │   ├── sync_protocol.rs # 传输无关的 manifest 同步协议
│   │   │   └── distribution.rs  # Symlink/Copy 落点逻辑
│   │   ├── orchestration/        # 跨领域用例编排
│   │   │   ├── sync.rs          # Sync Task 执行编排
│   │   │   ├── webdav_sync.rs   # WebDAV 上传/下载编排
│   │   │   └── auto_sync.rs     # DB hook / 事件驱动的防抖自动上传
│   │   ├── ports/                # 外部副作用边界（轻量接口）
│   │   │   ├── filesystem.rs    # 文件系统 / symlink / copy
│   │   │   ├── webdav.rs        # WebDAV 传输端口
│   │   │   └── tray.rs          # 托盘 / flyout surface 端口
│   │   ├── adapters/             # 端口的具体实现
│   │   │   ├── local_filesystem.rs
│   │   │   ├── reqwest_webdav.rs
│   │   │   └── tauri_tray.rs
│   │   ├── database/
│   │   │   ├── mod.rs           # Database 结构体、init、Mutex<Connection>
│   │   │   ├── schema.rs        # 建表 + 版本迁移
│   │   │   └── dao/             # 按领域拆分的查询方法（impl Database）
│   │   ├── store.rs             # AppState（Arc<Database> + services + orchestration）
│   │   └── error.rs             # AppError 枚举
│   ├── tauri.conf.json
│   └── Cargo.toml
├── prototype/                    # 视觉参考（只读，不参与构建）
├── docs/adr/
├── CONTEXT.md
└── package.json
```

## 实施计划

分四阶段推进，前端先行、后端渐进：

### 阶段一：前端全量页面（Mock）

- 初始化 Vite React 项目（`src-react/`，dev port 3000）
- 前端：App shell + 视图切换器 + 顶部 tab 导航
- 根据 `prototype/*.dc.html` 将所有页面迁移为 React 组件
- 6 个一级页面：Provider、Project、Skill、Prompt、Session、Sync + Settings
- 含所有交互：modal、overflow menu、agent matrix、拖拽排序、toast
- 数据来源：前端内存 mock（等价于 `nexus-data.js`），不经 IPC
- 原型已完整，此阶段目标是 1:1 复刻交互，不做功能裁剪
- 能通过 `pnpm dev` 在浏览器中验证全部页面和交互

### 阶段二：Tauri 项目骨架

- 初始化 Tauri 2 项目（`src-tauri/`，参考 cc-switch 的 `tauri.conf.json`、`package.json`）
- 配置 Tauri 使用已有 `src-react/` 前端：`devUrl` 指向 `http://localhost:3000`，`frontendDist` 指向 `../src-react/dist`
- Rust 侧：空 `lib.rs` + `main.rs`，能 `pnpm tauri dev` 打开窗口
- 不接真实 IPC，不引入 rusqlite；此阶段只验证桌面壳、窗口启动和前端静态资源加载

### 阶段三：IPC 接线 + 真实后端

- Rust 侧引入 rusqlite，建 schema，写 DAO + Services
- 前端切换到 `src-react/lib/api/` → `invoke()` → React Query
- 逐页面从 mock 切到真实数据源
- Services 层实现：Project 扫描、Skill SSOT、Session 索引、Distribution 规则
- Orchestration 层实现：WebDAV 同步、Sync Task 执行、auto-sync 编排
- 定时任务、auto-sync

### 阶段四：系统能力

- 系统托盘 + Provider flyout（borderless webview 复用卡片组件）
- Provider quota 按 surface 触发并发刷新（参考 cc-switch 托盘刷新模型）
- 自动启动、窗口状态恢复

## 核心架构模式

### 1. Command / Service 分离（参考 cc-switch）

Command 是薄的 Tauri 入口：校验输入 → 委托 service → 映射错误。业务逻辑不出现在 command 中。

```rust
#[tauri::command]
async fn scan_git_base_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<DiscoveredRepo>, String> {
    state.project_service.scan(&path).await.map_err(|e| e.to_string())
}
```

### 2. Service 依赖边界

领域 Service 不形成任意网状依赖。普通 Service 只负责本领域业务规则，可以依赖 `Database`、文件系统、传输接口等基础端口，但不直接依赖其他领域 Service 或 UI surface。

跨领域用例由显式 `orchestration/` 层组合，例如 `orchestration/sync.rs`、任务执行器或专用 workflow。系统托盘、窗口、flyout 等 UI surface 通过 command、事件或状态订阅接入，不进入领域 Service 依赖链。

`services/` 与 `orchestration/` 分目录维护：前者表达单领域规则，后者表达跨领域流程。不要用 `*_service` 命名掩盖编排逻辑。

### 3. 轻量 Ports & Adapters 边界

后端不全面改造成严格六边形架构，只在外部副作用边界使用轻量端口：

- 文件系统：扫描、读写、symlink/copy、路径校验
- WebDAV：`PUT` / `GET` / `MKCOL` 等远端传输
- 系统 surface：托盘、flyout、窗口事件

MVP 中 `Database` 不抽象为 port。rusqlite DAO 作为本地持久化基础设施直接被 Service 使用，避免为尚未变化的数据库边界提前制造 trait 和 adapter。

### 4. 前端 API 层（参考 cc-switch `src-react/lib/api/`）

每个领域一个 typed namespace 对象，组件不直接调 `invoke()`。

```typescript
// src-react/lib/api/projects.ts
export const projectsApi = {
  async list(): Promise<Project[]> {
    return invoke("list_projects");
  },
  async scanBaseFolder(path: string): Promise<DiscoveredRepo[]> {
    return invoke("scan_git_base_folder", { path });
  },
};
```

### 5. React Query 管理服务端状态

所有来自 Rust 后端的数据走 React Query。Mutation 触发相关 query 失效。

```typescript
export function useProjectsQuery() {
  return useQuery({ queryKey: ["projects"], queryFn: projectsApi.list });
}
```

### 6. 状态驱动视图切换（无路由库）

桌面应用不需要 URL 路由。`View` 联合类型 + `switch` 即可。

```typescript
type View = "provider" | "project" | "skill" | "prompt" | "session" | "sync" | "settings";
```

### 7. Manifest 同步协议（参考 cc-switch `sync_protocol.rs`）

- 上传：先传 artifacts（db.sql、skills.zip），最后传 manifest.json
- 下载：拉 manifest → 校验 SHA-256 → 应用快照
- 自动同步：SQLite `update_hook` → 防抖上传（1s 延迟，10s 最大等待）
- 单个 `Sync Task` 永远保持 `single source -> multiple targets`；如需配置文件多设备同步，使用两个显式反向 Task pair（例如 `A -> B` 与 `B -> A`），而不是让单个 Task 具备双向回流语义。

### 8. Provider 托盘 + Flyout（参考 cc-switch + agent-quota-monitor + Tauri 多窗口）

- Provider quota 刷新采用 cc-switch 的按需模型：托盘 hover/click 或页面操作触发，短时间节流，只刷新当前 surface 需要展示的可见 provider，并发执行后写穿进程内缓存。
- 每个 provider 独立隔离：失败不影响其他 provider 展示
- 官方 CLI Provider 优先只读既有凭据来源（例如 CLI 凭据文件或系统 Keychain），不接管第三方登录生命周期；手动配置的 Provider connection params 可进入本地数据库，但 UI 与日志必须脱敏。
- 托盘图标：单图标 + 右键菜单（快捷操作、退出）
- **Flyout 悬停详情**：Tauri 第二窗口（borderless webview），渲染与 Provider 页面相同的卡片组件
  - 悬停/点击托盘图标时定位到图标附近弹出
  - 窗口内容是同一个 React `<ProviderCard />` 组件，样式完全一致
  - 失焦自动关闭
  - 跨平台行为统一（Windows / macOS 均为 borderless webview 窗口）
- 不自绘原生窗口：相比 agent-quota-monitor 的 Win32 GDI 方案，webview 方案的优势是卡片只写一次、样式保证一致、天然跨平台

## 数据库设计

详见 [ADR 0002: 数据库 Schema 设计](./adr0002-数据库 Schema 设计.md)。

## 后果

### 正面

- 跨平台一套代码（Mac + Windows + Linux）
- 安装包 ~4-10 MB，vs Electron ~150 MB
- AI（Claude）对 React + Tailwind 前端生产力极高
- cc-switch 为每个主要子系统提供了经验证的模式
- SQLite FTS5 支撑未来 Session 全文检索
- WebDAV auto-sync via DB hook 优雅且防抖安全
- cc-switch 验证了托盘触发的 Provider quota 刷新、节流和缓存模型
- agent-quota-monitor 验证了 Provider 托盘展示、flyout 和图标渲染的可行性
- 文件系统、WebDAV、托盘等外部副作用被端口隔离，降低 Service 层测试和替换成本

### 负面

- Rust 学习曲线（通过 cc-switch 参考缓解）
- 调试跨两种语言（TS 前端 + Rust 后端）
- Tauri 插件生态小于 Electron
- rusqlite 需要 Mutex 包装（无原生 async SQLite）
- 轻量端口边界增加少量 trait / adapter 代码

### 中性

- 前端技术（React + Vite + Tailwind）与壳选型无关
- Prototype HTML/CSS 仅作视觉参考，不直接加载
- `Database` 在 MVP 不抽 port；未来只有在需要替换持久化或显著提升测试隔离时再重新评估

## 被否决的替代方案

| 替代 | 否决原因 |
|------|----------|
| Electron + Node.js | 150 MB 包体；cc-switch 证明 Tauri 对此类问题可行 |
| AppKit / SwiftUI | 仅 macOS，违反跨平台需求 |
| Flutter | AI 辅助生产力低；audio/native 交互痛点（来自外部参考） |
| JSON 文件（不用 SQLite） | 一旦需要搜索/聚合就必须迁移 |
| Zustand/Jotai | React Query 已覆盖服务端状态；多一层状态库无必要 |

## 参考来源

| 参考 | 路径 | 用途 |
|------|------|------|
| cc-switch | `D:\Sample\cc-switch\` | 整体架构、WebDAV sync、SQLite DAO、service 分层、前端模式、Provider quota 刷新模型 |
| cc-switch sync protocol | `src-tauri/src/services/sync_protocol.rs` | manifest 同步协议设计 |
| cc-switch schema | `src-tauri/src/database/schema.rs` | 迁移模式、表设计参考 |
| agent-quota-monitor | `D:\Workspace\agent-quota-monitor\` | Windows 托盘、flyout、图标渲染 |
| Agent Nexus prototype | `prototype/*.dc.html` | 视觉和交互参考 |
| Agent Nexus 领域模型 | `CONTEXT.md` | 实体定义与边界 |
