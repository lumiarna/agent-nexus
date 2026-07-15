# nexus-core Error Handling

## 基础类型

- 统一使用 `crate::error::{AppError, AppResult}`。
- `AppError` 包含 `Validation`、`Database`、`Io`、`Reconciliation`、`Internal`，并通过 serde tag/content 序列化给 Tauri 前端。参考 `src/error.rs`。
- `rusqlite::Error` 和 `std::io::Error` 已实现 `From`，service 中优先使用 `?` 传播。

## Validation 规则

- 用户输入、路径、ID、排序列表、任务不变量必须在 service 层校验，不能只靠前端。
- 共享校验 helper 放在 `services/util.rs`，例如 `required_trimmed`、`require_agent`；不要在每个 service 复制 trim/empty 检查。
- 错误文案应说明缺少什么或哪条规则违反，例如 `task group name is required`、`project order must include every project exactly once`。

## IO / 文件系统错误

- 文件系统扫描要区分 NotFound 与真实错误。参考 `services/distribution.rs` 中 placement 不存在时返回 `Ok(false)`。
- 执行 placement / copy / WebDAV 时，先准备计划和校验，再修改 DB 状态；失败时尽量回滚已经创建的 placement。

## Reconciliation 错误

- Project custom Skill 多 Placement intent 发生普通失败时先逆序补偿；补偿全部成功则保留原始 Validation / IO / Database kind。
- 只有补偿本身失败才返回 `AppError::Reconciliation`，并在失败后持久化 `skill_propagation_reconciliations` evidence；错误 message 必须包含 evidence ID。
- evidence 持久化也失败时，Reconciliation message 同时保留原始错误、补偿错误和 evidence 写入错误，不能相互覆盖。
- evidence 不是真相源，不触发启动恢复；相同 typed intent 可以幂等重试并在成功后 resolve evidence。

## Internal 错误

- `Internal` 用于锁中毒、创建后读回失败等不应由用户输入直接触发的状态。参考 `Database::connection` 和 `TaskLifecycle::create_task_group`。
- 不要用 `Internal` 包装普通 validation。

## 常见错误 / anti-pattern

- 在 service 中 `unwrap()` / `expect()` 处理用户资产路径、数据库结果或网络响应。
- 把所有错误压成 `String`，丢失 validation / io / database 分类。
- 在测试中只断言失败而不覆盖具体不变量；应像 `tests/sync_service.rs` 一样验证方向、action、status 等领域结果。
