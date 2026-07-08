# Code Reuse Thinking Guide

## 目标

在写代码前识别 Agent Nexus 已有的抽象，避免把同一条领域规则散落到多个层或多个文件。

## 先搜索再新增

新增 helper、配置常量、agent 名称、provider id、路径规则或 sync 规则前，先搜索现有实现：

```bash
grep -R "要修改或新增的值" src-react src-tauri crates/nexus-core docs CONTEXT.md
```

## 本项目优先复用的位置

- 前端 IPC：`src-react/src/lib/api/<domain>.ts`，不要在组件里直接 `invoke`。
- 前端 server state：`src-react/src/lib/query/<domain>.ts`，不要创建页面级服务端数据镜像。
- 前端纯规则：领域目录内的 dependency-free module，例如 `components/sync/taskRules.ts`。
- Rust 通用 helper：`crates/nexus-core/src/services/util.rs`、`paths.rs`、`system_open.rs`。
- Agent capability：`crates/nexus-core/src/services/agent_capabilities.rs` 与前端 `config/agents.ts`。
- Skill / Prompt 传播：`services/distribution.rs`，不要复制 Agent Matrix 不变量。
- Sync 生命周期：`services/sync/`，不要在 command 或 UI 重写领域规则。

## 需要停下来复用的信号

- 同一个 `Agent` 名称、配置目录或 prompt/skill 路径被手写第二次。
- 同一个 mutation 成功后需要失效多个 query。
- 同一个 `Project` list/string 配置需要 trim、去重、校验。
- 同一个 path display / `~` collapse / Windows separator 逻辑被复制。
- 同一个 Sync action/location/schedule 规则同时出现在 create 和 add task 表单。

## 常见 anti-pattern

- 为了快速实现，在 React 组件、Tauri command、core service 三层各写一份校验。
- 新增 Provider 时复制旧 provider 的 credential 读取和 HTTP 逻辑，却没有接入 provider_quota adapter 模式。
- 把 `Skill customSkillsDirs`、`Prompt extraPromptFiles`、`Session sessions_dir` 抽成一个统一结构；ADR-0003 已拒绝该方向。

## 完成前检查

- 是否已有测试覆盖复用模块？如没有，优先给纯 module 或 service 补测试。
- 是否更新了所有消费者，而不是只改当前页面？
- 是否仍符合 `CONTEXT.md` 的 canonical terminology？
