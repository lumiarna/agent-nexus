# Pull（Cloud→Local）增量跳过策略

## 问题

当前 `pull_cloud_to_local` 每次都会重新下载全部内容（单文件或目录），未利用 `task_file_state` 表实现增量跳过。对于大目录或高频 Pull 任务，这会造成不必要的带宽和磁盘写入开销。

## 现状

- Push 方向已通过 `task_file_state`（file_size + file_mtime）实现基于 size + mtime 的增量跳过
- Pull 方向目前每次都会重新下载全部内容

## 决策

当前实现优先保证 Pull 可用（Pull Once），增量跳过作为后续优化。

## 未来实现约定

### 方案 A：扩展 `task_file_state` 语义

将 `task_file_state` 同时用于记录 cloud 侧 metadata（而非仅 local source）：

- Pull 完成后，将 cloud 侧的 `getcontentlength` + `getlastmodified` 写入 `task_file_state`
- 下次 Pull 时，先列出远程目录，对比每个文件的 cloud metadata 与 `task_file_state`
- 若 size 和 mtime 均一致，跳过下载

### 方案 B：新增 `cloud_file_state` 表

保持 `task_file_state` 仅记录 local source metadata（与 Push 语义一致），新增 `cloud_file_state` 表专门记录 cloud 侧 metadata：

- schema：`task_id, rel_path, file_size, file_mtime, updated_at`
- Pull 完成后写入 cloud 侧 metadata
- 下次 Pull 时对比 `cloud_file_state`

## 备注

- 方案 A 改动更小（复用现有表），但 `task_file_state` 的语义会变得不统一（同一行可能是 local 或 cloud metadata）
- 方案 B 更干净，但需要新增 schema 和 migration
- 推荐方案 B，因为 schema 清晰，migration 成本在可接受范围内
