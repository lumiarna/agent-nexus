# Cross-Layer Thinking Guide

## 目标

Agent Nexus 的功能常跨越 React 前端、Tauri command、`nexus-core` service、SQLite、文件系统和 Cloud/WebDAV。跨层改动前先梳理数据流，避免只改一层导致语义漂移。

## 典型数据流

1. UI 组件：`src-react/src/components/<domain>/`。
2. Query / mutation：`src-react/src/lib/query/<domain>.ts`。
3. Typed IPC API：`src-react/src/lib/api/<domain>.ts`。
4. Tauri command：`src-tauri/src/commands/<domain>.rs`。
5. Core service：`crates/nexus-core/src/services/<domain>.rs` 或 deep module。
6. 持久化 / 外部副作用：`database/schema.rs`、filesystem placement、WebDAV、Provider API。
7. 测试：前端 `src-react/tests/`，后端 `crates/nexus-core/tests/`。

## 改字段 checklist

- Rust serde 类型是否新增/改名？
- Tauri command 参数和返回值是否仍是 `AppResult<T>`？
- 前端 `types/index.ts` 是否同步？
- `lib/api` 和 `lib/query` 是否同步？mutation 后 query invalidation 是否覆盖派生数据？
- SQLite schema / migration / tests 是否同步？
- 文案是否符合 `CONTEXT.md`：UI 用 `Cloud`，Agent 用 canonical names，Provider 不做 project-level quota。

## 改领域规则 checklist

- 规则的最终真相源是否在 `nexus-core` service，而不是前端？
- 前端纯规则是否只是 UX mirror，例如 `taskRules.ts` mirror 后端 `prepare_task`？
- 是否需要事务或文件系统回滚？参考 `distribution.rs::write_target` 与 Sync task lifecycle。
- 是否影响系统托管记录，例如 Session Backup task group？

## 常见跨层陷阱

- 只更新 UI type，忘记 core serde 返回字段。
- 只更新 core validation，忘记前端禁用态或提示，导致 UX 可选但提交失败。
- 只更新 Project 配置，忘记失效 Skill / Prompt / Session / Sync query。
- 把实现层 `WebDAV` 暴露到主内容 UI；主界面应使用 `Cloud`。
- 在 Windows 文档或测试说明中写裸 `cargo test -p nexus-core`，绕过 SQLite 动态链接包装。

## 验证策略

- 前端跨层改动：`pnpm typecheck` + 相关 `test:unit` / `test:component`。
- core 改动：Windows 下用 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`。
- schema/领域术语改动：对照 `CONTEXT.md`、`docs/design/Database Schema.md` 和相关 ADR。
