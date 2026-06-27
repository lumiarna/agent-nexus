# TaskLifecycle 拆出 Session Backup 调和 与 增量 File State 两个深模块

> 架构深化 issue（由 improve-codebase-architecture 探索得出）。推荐强度：**Worth exploring**。
> 词汇遵循 `CONTEXT.md`（Sync Task / Session Backup / Direction / Action）与 codebase-design（module / interface / depth / seam / locality）。

## 问题

`crates/nexus-core/src/services/sync/task_lifecycle.rs` 单文件 **1426 行**，`TaskLifecycle` 一个类型把多个本不相关的关注点压在一起：

| 关注点 | 位置 | 评价 |
| --- | --- | --- |
| Task Group / Task 列表查询 | `list_task_groups` 等 | 合理 |
| **Session Backup 调和**（按 Project 物化/修复系统 Copy 任务） | `reconcile_session_backups` `198-558`（≈360 行） | **过重，藏在读路径里** |
| Task 运行派发 | `run_task_operation` `559-629` | 三个 Direction 分支重复 |
| WebDAV push/pull | `Transfer` seam + free fn | seam 良好（见下） |
| Local→Local 拷贝 | `copy_local_to_local` 等 | 与 file-state 缠绕 |
| **增量 File State**（mtime 差分、跳过决策） | `load/refresh/save/should_skip_file` `981-1108` | **无 seam 的深逻辑** |
| symlink placement / SQL 行映射 | `create_*_link_placement` / `task_from_row` | 合理 |

其中 `Transfer` trait（`WebdavTransfer` + 测试用 `RecordingTransfer`）是个**已经做对的 seam**——两个 adapter ⇒ 真 seam，别动它。问题在另外两块深逻辑**没有 seam，被摊在自由函数里**。

### friction 1：读操作触发 360 行写调和

`list_session_backups`（153）开头就 `self.reconcile_session_backups()?`——一个「列表」读请求会先跑一段开事务、按 Project 物化/删除/修复系统 Copy 任务的写逻辑。读写边界被打穿，`reconcile` 又长达 360 行，是整个文件最难读的一块。Session Backup 在 `CONTEXT.md` 里是有名有姓的概念（每个 Project 默认物化一个 `Local __sessions/ → Cloud Session/{{project_key}}/` 的 Copy Task），这段逻辑理应是它自己的深 module，而不是 TaskLifecycle 的一个私有方法。

### friction 2：file-state 三处重复编排

`run_task_operation`（559-629）三个 Direction 分支里，「读 state → 跑 transfer → 回写 state」这段几乎逐字重复三遍：

```rust
let file_states = { let conn = self.db.connection()?; load_file_state(&conn, &task.id)? };
self.transfer.push_local_to_cloud(task, &settings, &file_states).await?;   // 仅这行不同
let source = resolve_local_path(&task.source)?;                            // pull 用 task.target
if source.is_dir() { refresh_file_state(&conn, &task.id, &source)?; }
else if source.is_file() { save_single_file_state(&conn, &task.id, &source)?; }
```

`load/refresh/save_single/should_skip/insert_recursive/file_mtime_epoch` 这一族（981-1108）是「增量同步的文件状态」这一个深概念，但它以散装自由函数形态嵌在编排里，调用约定（什么时候 load、什么时候 save、dir vs file）靠每个分支自己记，没有 locality。`[[260626-1740-pull-incremental-skip]]` 与 `[[260626-1740-pull-task-file-state-save]]` 记录的正是这块语义当前在 Pull 方向「只存不读、语义不清」——根因就是 file-state 没有自己的 interface 来集中表达约定。

### deletion test

- 删掉一个 `FileState` module → mtime 差分、跳过判定、dir/file 回写约定会在三个 Direction 分支各自重现 ⇒ 真 module。
- 删掉一个 `SessionBackupReconciler` → 按 Project 物化系统任务的规则会散回 list 读路径 ⇒ 真 module。

## What to build

从 `TaskLifecycle` 抽出两个深 module，让 `TaskLifecycle` 退回成「编排者」：

### 1. `FileState`（增量文件状态）

把 `load_file_state` / `refresh_file_state` / `save_single_file_state` / `should_skip_file` / `insert_file_state_recursive` / `file_mtime_epoch` 收敛到一个 module，interface 小：

```rust
impl FileState {
    fn load(conn, task_id) -> FileStateMap;
    /// 跑完一次成功同步后，按 source 是 dir/file 自动回写——吃掉三处重复的 if-dir/else-file。
    fn record(conn, task_id, source: &Path) -> AppResult<()>;
    fn should_skip(source, rel_path, &map) -> bool;
}
```

`run_task_operation` 三个分支缩成「`let map = FileState::load(...)` → transfer → `FileState::record(..., effective_source)`」，其中 `effective_source` 由 Direction 决定（push=source，pull=target，local=source）。这也给 `[[260626-1740-pull-incremental-skip]]` 的方案 B（cloud 侧 state）一个干净的落点：未来 Pull 的 cloud-metadata 差分是 `FileState` 的实现细节，不再污染编排层。

### 2. `SessionBackupReconciler`（系统 Copy 任务物化）

把 `reconcile_session_backups`（198-558）整体搬进一个 module，owns「按 Project 计算应存在的系统 Copy Task、增删、修复路径」的全部规则。`list_session_backups` 改为：先显式调一次 `reconciler.reconcile()`，再做纯读查询——读写在调用点可见，而不是藏在 list 内部。

## Suggested shape

- **保留 `Transfer` seam 不变**：它有两个 adapter（Webdav + 测试 Recording），是真 seam。两个新 module 与它正交。
- **`FileState` 的 interface 即测试面**：`should_skip` / `record` 应能脱离网络、脱离 `TaskLifecycle` 单测（给定 conn + 临时目录即可）——当前这些逻辑只能透过整条 run 路径间接测到。
- **`SessionBackupReconciler` 接受 `&Connection`/`&Database`，不自建**：纯函数式地「给定当前 Project 集合与既有系统任务，算出增删」，便于直接断言物化结果，而不必跑完整 list。
- **不要为 push/pull/local 三个方向再造 seam**：它们已在 `Transfer` 后面；这里只是把 file-state 与 reconcile 两块**抽成 module**，不是加新的 trait 抽象层。

## Before / After

```text
BEFORE  TaskLifecycle (1426 行) ──────────────────────────
  list_session_backups ──(读触发)──> reconcile_session_backups (360 行写)
  run_task_operation ── load/refresh/save 三分支逐字重复
  file-state = 散装自由函数，调用约定靠记

AFTER
  TaskLifecycle  ── 编排者（薄）
    ├─ SessionBackupReconciler.reconcile()   ← 显式调用，读写边界清晰
    ├─ Transfer（既有 seam，不动）
    └─ FileState{ load / record / should_skip }  ← 增量语义集中，可独立单测
```

## Acceptance criteria

- [ ] `run_task_operation` 三个 Direction 分支不再各自手写 `if source.is_dir()/else file` 的 state 回写；统一走 `FileState::record`。
- [ ] `FileState` 的跳过与回写逻辑可在不跑网络 transfer 的前提下单测。
- [ ] `list_session_backups` 中 reconcile 是显式调用，函数体回归为「reconcile 后纯读」。
- [ ] `reconcile_session_backups` 的物化规则迁入 `SessionBackupReconciler`，并能对「给定 Project 集合算出应增删的系统任务」单独断言。
- [ ] `Transfer` seam 签名不变；`tests/sync_service.rs`（2430 行）全绿。
- [ ] 对外行为不变：系统 Session Backup 任务的物化结果、Copy 语义、增量跳过现状均不回归。

## Out of scope

- 不实现 Pull 增量跳过本身（仍由 `[[260626-1740-pull-incremental-skip]]` 跟踪）——本 issue 只提供 `FileState` 这个干净落点。
- 不改 Session Backup 的默认 source/target/schedule 语义（`CONTEXT.md` 定义不变）。
- 不动 `Transfer` / WebDAV 实现，不改 symlink placement 逻辑。
- 不解决 `[[260626-1740-pull-partial-failure-reporting]]` / `[[260626-1740-pull-directory-detection-trailing-slash]]`，它们是 transfer 内部行为，独立推进。

## Notes

两个抽取彼此独立，可分两个 PR。建议先抽 `FileState`（改动局部、被三处复用、收益直接），再抽 `SessionBackupReconciler`（涉及读写边界调整，需配套 `tests/sync_service.rs` 验证物化不回归）。这条 issue 是上述四个 `260626-1740-pull-*` 的**架构母题**：它们是具体行为缺口，本 issue 给出承载这些行为的正确 module 形状。
