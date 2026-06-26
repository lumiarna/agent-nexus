# Pull（Cloud→Local）目录检测鲁棒性

## 问题

当前 `pull_cloud_to_local` 通过 `task.source.ends_with('/')` 判断是否为目录下载：

```rust
if task.source.ends_with('/') {
    // 目录递归下载
} else {
    // 单文件下载
}
```

如果用户创建 Pull 任务时未在 cloud path 末尾加 `/`，会被当成单文件处理，导致下载失败（`get_bytes` 会得到 404 或 HTML 目录列表）。

## 现状

- WebDAV `get_bytes` 对目录路径会返回 404
- WebDAV `list_directory` 对文件路径会返回空列表或错误
- 用户在 UI 中需要手动记住加 `/` 的约定

## 决策

当前实现依赖用户遵守 trailing slash 约定。未来应通过 PROPFIND 探测资源类型。

## 未来实现约定

### 实现步骤

1. 在 `pull_cloud_to_local` 中，先通过 `webdav::list_directory` 或自定义 `propfind` 探测 source 资源类型
2. 若返回集合（collection），按目录递归下载
3. 若返回文件（非 collection），按单文件下载
4. 若 404，返回 Validation error

### 权衡

- 额外一次 PROPFIND 请求的开销 vs 用户不再需要记忆 trailing slash 约定
- 推荐实现：增加探测逻辑，失败时 fallback 到现有 trailing slash 判断（向后兼容）

## 备注

- 同样的探测逻辑也可以用于 Push 方向的 source 类型检测（Local source 是文件还是目录）
- 当前 Local source 使用 `fs::metadata` 判断，比 WebDAV 探测更可靠
