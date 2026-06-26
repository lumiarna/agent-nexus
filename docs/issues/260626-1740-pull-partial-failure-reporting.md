# Pull（Cloud→Local）部分失败报告

## 问题

当前 `pull_cloud_to_local` 在目录递归下载时，遇到单个文件下载失败会直接返回 `Err(AppError)`，导致整批 Pull 中断，已下载的文件不会回滚，但用户不知道哪些文件成功、哪些失败。

## 现状

```rust
for entry in entries {
    if entry.is_collection {
        // 递归下载子目录
    } else {
        let bytes = webdav::get_bytes(...).await?; // 单个文件失败即中断
        fs::write(&child_target, bytes)?;
    }
}
```

- `?` 传播错误，导致整批失败
- 已写入的本地文件不会自动清理

## 决策

当前实现优先保证简单可用，部分失败报告作为后续优化。

## 未来实现约定

### 方案

1. 收集每文件下载结果（成功/失败 + 错误信息）
2. 全部文件尝试完成后，若有任何失败，返回汇总错误
3. 汇总格式示例：
   ```
   Pull completed with 2 failures:
   - a.txt: 404 Not Found
   - sub/b.txt: Connection timeout
   ```

### 实现建议

- 定义 `PullResult` 结构体：`{ successes: Vec<String>, failures: Vec<(String, AppError)> }`
- 在 `pull_cloud_to_local` 中返回 `AppResult<PullResult>`
- `run_task_operation` 中根据 `PullResult` 决定最终状态（全部成功 → Ok, 有失败 → 汇总错误）

## 备注

- Push 方向目前也有类似问题（`push_local_directory_to_cloud` 中的 `?` 会中断整批上传）
- 可考虑统一 Push 和 Pull 的失败报告格式
- 若实现回滚（删除已下载/上传的部分文件），复杂度会显著增加，暂不推荐
