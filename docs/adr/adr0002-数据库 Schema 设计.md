# ADR 0002: 数据库 Schema 设计

- **状态**: 已接受
- **日期**: 2026-06-16
- **关联**: [ADR 0001](./adr0001-产品技术栈和架构设计.md)（技术栈选型确定 rusqlite）

## 背景

Agent Nexus 的领域模型（见 `CONTEXT.md`）包含以下核心实体：

- **Project** — Git 仓库，是 Session、project Skill 和 Sync 的上下文边界
- **Skill** — 可传播的共享能力资产，有 global / project 两种 scope
- **Prompt** — 全局提示资产
- **Provider** — 外部服务身份，提供 quota 观测
- **Session** — 可搜索、可归档的会话内容（正文留在文件系统，数据库只存索引）
- **Sync** — 由 Task Group / Task 组成的单向任务工作台
- **Distribution** — Skill/Prompt 从 canonical source 到 target agent 的传播关系（Agent Matrix）

本 ADR 定义初始 schema（v1），采用 rusqlite 直写 SQL，不引入 ORM 框架。

## 决定

### 设计原则

1. **实体对齐 CONTEXT.md** — 每张表对应一个明确的领域概念，命名取自 glossary
2. **传播关系独立建表** — Distribution 是 Skill/Prompt 到 Agent 的 many-to-many 关系，拆为 `skill_distributions` / `prompt_distributions`，而非 JSON 字段
3. **Session 正文不入库** — `session_index` 只存元数据 + 摘要；正文留在文件系统或 WebDAV
4. **FTS 用 content-sync 模式** — `session_fts` 是 `session_index` 的外部内容 FTS5 表，需手动同步
5. **JSON 字段仅用于弱结构数据** — `tasks.targets`（路径数组）、`providers.connection_params`（可变键值对）
6. **时间戳统一用 Unix epoch 整数（秒）** — 不用 ISO 字符串
7. **主键统一用 UUID（TEXT）** — 除自增辅助表（`provider_windows`）外

### Schema（v1）

```sql
-- ─── 版本追踪 ─────────────────────────────────────────────────

CREATE TABLE schema_version (
    version INTEGER NOT NULL
);
INSERT INTO schema_version (version) VALUES (1);

-- ─── Workspace ────────────────────────────────────────────────

-- Project: 被 Agent Nexus 收录的 Git repository root
-- CONTEXT.md: Project Key 是跨设备归并的稳定身份键，默认取目录名
CREATE TABLE projects (
    id TEXT PRIMARY KEY,                              -- UUID
    name TEXT NOT NULL,                               -- 显示名（初始 = 目录名）
    key TEXT NOT NULL UNIQUE,                         -- 稳定身份键（= 目录名）
    path TEXT NOT NULL,                               -- 当前本地仓库路径（可变）
    status TEXT NOT NULL DEFAULT 'active'             -- active | stale | hidden
        CHECK (status IN ('active', 'stale', 'hidden')),
    sessions_dir TEXT NOT NULL DEFAULT '__sessions',  -- 会话目录模板
    sort_index INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Git Base Folder: 用于自动发现 Project 的扫描根目录（不是 Project 本身）
CREATE TABLE git_base_folders (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    added_at INTEGER NOT NULL
);

-- ─── Assets ──────────────────────────────────────────────────

-- Skill: 可被 agent 消费的共享能力资产
-- CONTEXT.md: 同时支持 global 与 project 两种 Scope
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project_id TEXT,                                  -- global 时为 NULL
    description TEXT,
    canonical_path TEXT NOT NULL,                     -- Canonical Source 文件路径
    disabled INTEGER NOT NULL DEFAULT 0,              -- disable-model-invocation
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- Skill 传播关系（Agent Matrix 的持久化）
-- CONTEXT.md: 每行必须且只能有一个 source
CREATE TABLE skill_distributions (
    skill_id TEXT NOT NULL,
    agent TEXT NOT NULL,                              -- Agent 名：Agents | Claude Code | CodeX | Copilot | OpenCode
    role TEXT NOT NULL CHECK (role IN ('source', 'target', 'none')),
    target_path TEXT,                                 -- target 的 Placement 路径（source/none 时为 NULL）
    PRIMARY KEY (skill_id, agent),
    FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

-- Prompt: 全局提示资产
-- CONTEXT.md: MVP 只覆盖 global prompt file
CREATE TABLE prompts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    canonical_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Prompt 传播关系
CREATE TABLE prompt_distributions (
    prompt_id TEXT NOT NULL,
    agent TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('source', 'target', 'none')),
    target_path TEXT,
    PRIMARY KEY (prompt_id, agent),
    FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
);

-- Provider: 提供 quota 信息与 credential source 的外部服务身份
-- CONTEXT.md: 全局资源，不做 project-level 归因
CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    plan TEXT,
    status TEXT NOT NULL CHECK (status IN ('available', 'expired', 'failed', 'nocreds')),
    credential_source TEXT,                           -- 凭据来源描述
    connection_params TEXT,                           -- JSON: workspace id 等连接参数
    is_agent INTEGER NOT NULL DEFAULT 0,             -- 是否是 agent provider（影响 tray 展示）
    sort_index INTEGER,                              -- Display Order（拖拽排序）
    card_visible INTEGER NOT NULL DEFAULT 1,         -- Card Visibility
    tray_visible INTEGER NOT NULL DEFAULT 1,         -- Surface Preference（托盘）
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Provider Quota Window: 每个 provider 的用量时间窗口
CREATE TABLE provider_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    label TEXT NOT NULL,                              -- 如 "5-hour limit"、"Weekly limit"
    used_percent INTEGER NOT NULL,
    reset_label TEXT,                                 -- 如 "Resets in 2h 14m"
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

-- ─── Session ─────────────────────────────────────────────────

-- Session Index: 元数据索引（正文留在文件系统）
-- CONTEXT.md: Session 既有 Local 视图也有 Cloud 视图
CREATE TABLE session_index (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    file_path TEXT NOT NULL,                          -- 相对于 sessions_dir 的路径
    excerpt TEXT,
    source TEXT NOT NULL CHECK (source IN ('local', 'cloud', 'both')),
    size_bytes INTEGER,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- Session 全文检索（FTS5 外部内容模式）
CREATE VIRTUAL TABLE session_fts USING fts5(
    title, excerpt,
    content=session_index, content_rowid=rowid
);

-- ─── Sync ────────────────────────────────────────────────────

-- Task Group: Sync Task 的组织容器
-- CONTEXT.md: 用于创建、排序、批量查看与批量触发，不承载执行方向语义
CREATE TABLE task_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_index INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Task: 具体的单向执行任务
-- CONTEXT.md: 方向与类型定义在 Task 层；可独立配置 manual 或 CRON 调度
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL,
    direction TEXT NOT NULL                           -- Distribution | Backup | Restore/Pull
        CHECK (direction IN ('Distribution', 'Backup', 'Restore/Pull')),
    action TEXT NOT NULL CHECK (action IN ('symlink', 'copy')),
    source TEXT NOT NULL,
    targets TEXT NOT NULL,                            -- JSON array of target paths
    schedule TEXT NOT NULL DEFAULT 'manual',          -- 'manual' | CRON 表达式
    sort_index INTEGER,
    last_run_at INTEGER,
    last_status TEXT CHECK (last_status IN ('ok', 'failed', 'never') OR last_status IS NULL),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (group_id) REFERENCES task_groups(id) ON DELETE CASCADE
);

-- ─── Settings ────────────────────────────────────────────────

-- 全局键值设置（WebDAV 配置、tray_metric_mode 等）
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### 索引

```sql
CREATE INDEX idx_skills_scope ON skills(scope);
CREATE INDEX idx_skills_project ON skills(project_id) WHERE project_id IS NOT NULL;
CREATE INDEX idx_session_index_project ON session_index(project_id);
CREATE INDEX idx_session_index_source ON session_index(source);
CREATE INDEX idx_tasks_group ON tasks(group_id);
CREATE INDEX idx_provider_windows_provider ON provider_windows(provider_id);
```

### 初始设置种子

```sql
-- Tray Metric Mode: CONTEXT.md 定义为全局统一配置
INSERT INTO settings (key, value) VALUES ('tray_metric_mode', 'Remaining');
-- WebDAV
INSERT INTO settings (key, value) VALUES ('webdav_url', '');
INSERT INTO settings (key, value) VALUES ('webdav_user', '');
INSERT INTO settings (key, value) VALUES ('webdav_pass', '');
```

## 迁移策略

参考 cc-switch 的 `database/schema.rs` 模式：

1. 启动时读 `schema_version` 表
2. 如果版本低于当前代码版本，依次执行迁移函数（`migrate_v1_to_v2`、`migrate_v2_to_v3` ...）
3. 迁移前自动备份当前数据库文件
4. 每次迁移是一个独立事务

```rust
// src-tauri/src/database/schema.rs
const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    let current = get_schema_version(conn)?;
    if current < 1 { migrate_to_v1(conn)?; }
    // if current < 2 { migrate_v1_to_v2(conn)?; }
    Ok(())
}
```

## 不做的事

- **不用 ORM** — rusqlite 直写 SQL，DAO 方法按领域拆文件（`impl Database` 块）。原因：ORM 在 Rust 生态（diesel/sea-orm）增加编译时间和 schema 同步复杂度，对本地桌面应用收益不足。
- **不做双向同步** — `tasks.direction` 只有单向值，遵循 CONTEXT.md "不合并成一个双向任务" 的约束。
- **不将 Session 正文入库** — 索引 + FTS 搜索摘要即可，正文通过文件路径按需读取。
- **不做 soft delete** — `projects.status = 'hidden'` 是显式状态，不是删除标记；真正删除直接 `DELETE`。

## 后果

### 正面

- Schema 严格对齐 CONTEXT.md 词汇表，代码与领域语言一致
- 传播关系独立建表，Agent Matrix 的 CRUD 操作清晰
- FTS5 为未来 Session 全文检索做好准备
- 迁移策略简单可靠（cc-switch 已验证）

### 负面

- JSON 字段（`targets`、`connection_params`）无法被 SQLite 索引或约束
- 手写 SQL 需要人工保证类型安全（无编译期校验）
- FTS content-sync 模式需要在 INSERT/UPDATE/DELETE session_index 时手动同步 FTS 表

### 中性

- Agent 名用字符串而非枚举表，因为 Agent 集合是固定的 canonical order（`CONTEXT.md: Display Order`），不需要额外一张表

## 参考

- CONTEXT.md 中与 schema 相关的领域定义：Project / Project Key / Skill / Prompt / Provider / Session / Sync / Task Group / Task / Distribution / Agent Matrix
- cc-switch schema 模式：`D:\Sample\cc-switch\src-tauri\src\database\schema.rs`
- cc-switch DAO 模式：`D:\Sample\cc-switch\src-tauri\src\database\dao\`
