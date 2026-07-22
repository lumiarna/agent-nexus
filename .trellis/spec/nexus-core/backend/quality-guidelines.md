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

## Scenario: 共享本地路径与系统打开边界

### 1. Scope / Trigger
- Trigger：修改任何本地路径输入/展示、home 目录语义、Provider credential path，或 Open / Reveal 行为。

### 2. Signatures
- `services::paths::{home_dir, resolve_local_path, collapse_home}`
- `services::system_open::{open_path, reveal_path}`

### 3. Contracts
- `resolve_local_path` 是本地路径字符串展开的唯一入口；`~`、`%APPDATA%`、`%LOCALAPPDATA%` 不得在领域 service 或 command 中另写解析器。
- `home_dir` 是 home 语义的唯一来源：Windows 优先原生 `USERPROFILE`，缺失/空值时才回退 `HOME`；非 Windows 使用 `HOME`。Provider、Project、Sync、Skill/Prompt 与 `collapse_home` 必须共享该规则。
- `collapse_home` 是绝对 home path 转 `~` 展示的唯一入口，不得直接读取 `HOME` 后自行 `strip_prefix`。
- 所有路径 Open / Reveal 副作用必须经过 `system_open`；`open_path` 与 `reveal_path` 都必须在启动 `open` / `explorer` / `xdg-open` 前拒绝不存在目标。
- 领域层仍负责额外语义，例如 Agent Config Root 必须是目录；`system_open` 的通用存在性保护属于副作用边界，即使形成防竞态的二次检查也不能删除。

### 4. Tests Required
- `services::paths`：Windows `USERPROFILE` 优先于 Git Bash 风格 `HOME=/c/Users/...`，并覆盖 `resolve_local_path` 与 `collapse_home`。
- 所有修改 `HOME` / `USERPROFILE` 的测试必须同时保存和恢复两者（优先 RAII guard，确保 panic 时也恢复）。同一 integration-test executable 内，修改环境的测试及会读取 home/path display 的测试必须共用 `serial_test` 锁；只给 writer 标 `#[serial]` 仍会与未标注 reader 并发并产生不稳定结果。
- `services::system_open`：Open 与 Reveal 都以缺失目标做纯失败测试，证明不会启动系统 handler。

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
