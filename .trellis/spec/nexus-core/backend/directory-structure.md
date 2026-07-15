# nexus-core Directory Structure

## 适用范围

适用于 `crates/nexus-core/`。这是 Agent Nexus 的领域 crate，不依赖 Tauri，可独立测试。

## 本项目结构

```text
crates/nexus-core/src/
├── lib.rs                       # pub mod database / error / services
├── error.rs                     # AppError / AppResult
├── database/
│   ├── mod.rs                   # Database: Mutex<rusqlite::Connection>
│   └── schema.rs                # CURRENT_SCHEMA_VERSION + migrate_to_vN
└── services/
    ├── mod.rs
    ├── util.rs / paths.rs / system_open.rs
    ├── agent_capabilities.rs
    ├── projects.rs / skills.rs / prompts.rs / sessions.rs
    ├── distribution.rs          # Agent-sourced Skill/Prompt 单 Placement deep module
    ├── project_custom_skill_propagation.rs # Project custom Skill eager read + intent/补偿
    ├── placement.rs / symlink.rs
    ├── provider_quota/          # Provider adapter + provider-specific modules
    ├── sync.rs / sync/          # Task lifecycle, file state, copy, reconciler
    ├── cron.rs / webdav.rs
    └── outbound_request_log.rs
```

测试在 `crates/nexus-core/tests/`，按服务领域组织，例如 `project_service.rs`、`skill_service.rs`、`sync_service.rs`。

## 模块边界

- `services/<domain>.rs` 承载领域 service；跨领域通用 helper 放入 `util.rs`、`paths.rs`、`system_open.rs`，不要复制。
- Agent-sourced Skill / Prompt 单 Placement 传播共享 `services/distribution.rs`；Project custom Skill 的 Global / Project 多 Placement 编排留在私有 `project_custom_skill_propagation.rs`，不要泛化成 Prompt/Session 共同 seam。
- Sync 的复杂生命周期拆到 `services/sync/` 子模块；`sync.rs` 暴露 public service 和类型。
- Provider quota 已按 `provider_quota/` 子目录组织；新增 provider 应放入 `providers/` 并通过 adapter/registry 接入。

## 领域建模规则

- 遵守 `CONTEXT.md`：`Project` 是 Git repository root，不是任意 folder；`Provider` 是 global resource；`Session` 是 Archivable Content，不进入 Agent Matrix。
- 遵守 ADR-0003：Skill `customSkillsDirs`、Prompt `extraPromptFiles`、Session `sessions_dir` 是三种不同形态，不要抽象成统一 `custom sources` Vec。
- Agent capability surface 使用 canonical names；参考 `services/agent_capabilities.rs`。

## 常见错误 / anti-pattern

- 在多个 service 中复制路径展开、时间戳、agent 校验等 helper。
- 为 `Database` 提前抽 trait port；架构文档明确当前 rusqlite 直接作为本地持久化基础设施。
- 在 `nexus-core` 引入 Tauri 类型或 Tauri runtime。
