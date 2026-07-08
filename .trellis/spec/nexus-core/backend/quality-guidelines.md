# nexus-core Quality Guidelines

## 必守领域约束

- `CONTEXT.md` 是领域真相源。不要把 `Agent` 误建模为 model/provider/account；不要把 `OpenCode Go` 当 `Agent`；不要把 `Provider quota` 做 project-level 归因。
- `Distributable Asset` 主要是 `Skill` / `Prompt`；`Session` 是 `Archivable Content`，不进入 Agent Matrix。
- `Generic Agent` 是 Agent Matrix / default global entry 的 canonical-leftmost baseline；display preferences 不能把它禁用。
- `Distribution` 是 Local→Local 单向关系，不是双向同步；`Sync Task` 永远 `1 source → 1 target`。
- `Project Key` 是稳定身份，`Project Path` 可变；扫描发现同 key 新路径应续认同一 Project。

## 测试模式

- 服务级集成测试放在 `crates/nexus-core/tests/`，按领域文件组织。
- 使用 `tempfile::TempDir` 创建 Git repo / 文件系统 fixture；参考 `tests/sync_service.rs` 的 `git_repo` helper。
- Windows symlink 权限特殊：测试中目录链接使用 `#[cfg(windows)] Junction`、非 Windows 使用 `Symlink`，参考 `tests/sync_service.rs`。
- Provider / WebDAV 等外部边界用 fake server / fake adapter，不让核心测试依赖真实网络。

## 验证命令

- Windows 下不要裸跑 `cargo test -p nexus-core`；遵守 `GOTCHAS.md`：

```bash
pnpm rust:test
# 或
node scripts/with-sqlite.mjs cargo test -p nexus-core
```

- 单文件格式修复可用：`cargo fmt --all -- <path>`；最终格式验证再跑 `pnpm rust:fmt`。

## 代码组织质量

- 重复 helper 上提到 `services/util.rs`、`paths.rs`、`system_open.rs`。
- 深模块优先：Distribution、Provider quota derive、cron、Transfer seam 已是本项目模式，不要在外层散落重复规则。
- 新增外部副作用时，只在必要边界加轻量 port；不要全面六边形化，也不要为 `Database` 造 trait。

## Scenario: Agent display preferences invariants

### 1. Scope / Trigger
- Trigger: 修改 `AgentDisplayPreferences`、Agent capability surface、Settings Agent toggle，或任何会影响 Agent Matrix enabled/disabled 集合的逻辑。

### 2. Signatures
- `crates/nexus-core/src/services/app_config.rs`
  - `AgentDisplayPreferences { disabled: Vec<String>, default_global_entry_agent: Option<String> }`
  - `normalize_agent_names(names: Vec<String>) -> AppResult<Vec<String>>`
  - `normalize_default_global_entry_agent(agent: Option<String>, disabled: &[String]) -> AppResult<Option<String>>`
- Tests：`crates/nexus-core/tests/app_config.rs`

### 3. Contracts
- `disabled` 里的名称必须是 canonical Agent name，不能为空、不能重复、不能是未知 Agent。
- `Generic Agent` 永远保持 enabled；service validation 必须拒绝把它写入 `disabled`。
- `default_global_entry_agent` 必须指向已知、Skill-capable、且当前未 disabled 的 Agent；若传入的是 disabled Agent，则规范化为 `None`。
- 这些不变量由 backend service 兜底，不能只依赖 Settings UI。

### 4. Validation & Error Matrix
- `disabled` 包含空字符串 -> `Validation("disabled agents cannot contain empty names")`
- `disabled` 包含 `Generic Agent` -> `Validation("Generic Agent cannot be disabled")`
- `disabled` 包含未知名称 -> `Validation("unknown agent: ...")`
- `disabled` 含重复名称 -> `Validation("disabled agents contains duplicate: ...")`
- `default_global_entry_agent` 指向未知或非 Skill-capable Agent -> `Validation(...)`
- `default_global_entry_agent` 同时在 `disabled` 里 -> 自动清成 `None`

### 5. Good/Base/Bad Cases
- Good: 用户禁用 `Copilot`，服务保存 `disabled = ["Copilot"]`；若默认 Global entry 也是 `Copilot`，则自动清成 `None`。
- Base: 用户不设置 `default_global_entry_agent`，前端回退到 canonical-leftmost `Generic Agent`。
- Bad: 只在前端把 Generic Agent toggle 灰掉，但后端仍接受 `disabled = ["Generic Agent"]`，导致脏配置能从旧版本 UI / 手工写入流入数据库。

### 6. Tests Required
- `crates/nexus-core/tests/app_config.rs`
  - reject disabling `Generic Agent`
  - reject unknown agent names
  - clear disabled default global entry agent to `None`
- 变更 `agent_capabilities.rs` canonical list 时，同时更新 capability/order tests，避免 preferences validation 与 capability source 漂移。

### 7. Wrong vs Correct
#### Wrong
```rust
if agent_by_name(name).is_none() {
    return Err(AppError::Validation(format!("unknown agent: {name}")));
}
```
- 只检查“是否存在”，没保护 `Generic Agent` 常驻 enabled。

#### Correct
```rust
if name == "Generic Agent" {
    return Err(AppError::Validation(
        "Generic Agent cannot be disabled".to_string(),
    ));
}
if agent_by_name(name).is_none() {
    return Err(AppError::Validation(format!("unknown agent: {name}")));
}
```
- backend service 明确守住产品不变量。

## 常见错误 / anti-pattern

- 把 UI 便利模型写入领域模型，例如把多 target form 直接落成一个 multi-target task。
- 把 Prompt extra file 建成新的 `Project Prompt Custom Source`；ADR-0003 明确它继承 Agent namespace。
- 把 Session Directory 改成 Vec；ADR-0003 明确 Session 始终单值。
