# Pull（Cloud→Local）任务完成后 file state 保存

## 问题

当前 `run_task_operation` 在 Pull 成功后也会执行 `refresh_file_state` 或 `save_single_file_state`，保存的是 **local target** 的 file state。但 Pull 方向的增量跳过逻辑尚未实现，导致这些保存的 state 暂时不会被读取。

## 现状

```rust
("Cloud", "Local") => {
    // ... pull ...
    let source = resolve_local_path(&task.target)?;
    if source.is_dir() {
        let conn = self.db.connection()?;
        refresh_file_state(&conn, &task.id, &source)?;
    } else if source.is_file() {
        let conn = self.db.connection()?;
        save_single_file_state(&conn, &task.id, &source)?;
    }
    Ok(TaskRunStatus::Ok)
}
```

- `task_file_state` 的语义是「最近一次成功同步后的本地 source 状态」
- 对于 Pull 来说，保存的是 target 的本地状态，而非 cloud source 的状态

## 决策

当前实现保留了 file state 保存逻辑（与 Push 对称），但增量跳过尚未实现，因此这些 state 暂时不会被读取。

## 未来实现约定

### 与增量跳过的关系

当 `docs/issues/260626-1740-pull-incremental-skip.md` 实现时，需要决定：

1. **方案 A（扩展 `task_file_state` 语义）**：
   - Pull 完成后保存 cloud metadata（size + mtime）到 `task_file_state`
   - 下次 Pull 时读取并对比 cloud metadata
   - 这意味着 `task_file_state` 同一行在不同 direction 下含义不同

2. **方案 B（新增 `cloud_file_state` 表）**：
   - Pull 完成后保存 cloud metadata 到 `cloud_file_state`
   - `task_file_state` 仅用于 Push 方向的 local source 状态
   - Pull 完成后不再保存 local target 的 file state（除非需要支持双向同步）

## 备注

- 推荐方案 B（新增表），语义更清晰
- 当前代码中 `save_single_file_state`/`refresh_file_state` 的调用在 Pull 实现中可暂时保留，作为 future-proofing；当增量跳过实现时再决定是否移除
