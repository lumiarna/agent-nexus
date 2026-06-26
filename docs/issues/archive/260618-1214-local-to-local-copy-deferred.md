# Local to Local copy 延后支持

## 问题

`src-tauri/src/services/sync.rs` 第 396-398 行，`run_task_operation` 的 `("Local", "Local")` 分支返回 stub 错误 `"Local to Local copy is not implemented yet"`。但 `CreateTaskInput` 允许 `source_type=Local, target_type=Local, action=Copy` 通过校验（`prepare_task` 仅拒绝 Cloud→Cloud 与非本地的 Symlink），用户能创建此类任务却无法运行。

## 决策

暂不实现。理由：本地→本地同步场景下 `Symlink`（已实现）几乎总是更优——零数据冗余、始终一致、无覆盖风险。Copy 仅在「源会变动但想冻结快照」或「跨卷/跨设备无法 symlink」时才必要，当前无此需求。

## 未来实现约定

若实现，语义已确认：

- 目录复制采用 `cp -r` 语义：target 已存在目录时，源目录嵌入其名（`dst/src/...`），而非 rsync 风格平铺内容。
- 覆盖策略**不静默覆盖**：先删 target（放回收站），再复制，避免误覆盖用户文件。

## 备注

- 回收站 API 非标准库提供，需平台特定调用（Windows `SHFileOperation` + `FOF_ALLOWUNDO` 或 `IFileOperation`、macOS `NSWorkspace.recycleURLs`、Linux 依赖 trash spec）。实现成本高于 cloud 路径的静默覆盖，是延后决策的次要从因。
- 同一 `run_task_operation` 中 `("Cloud", "Local")` 也是 stub（sync.rs:393-395），未来实现本地落盘时可与本项共享写入/覆盖约定。
