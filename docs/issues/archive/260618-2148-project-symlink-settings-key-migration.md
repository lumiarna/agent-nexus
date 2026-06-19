# Project Symlink settings key 迁移

## What to build

将 Project Symlink inventory 的配置 key 从历史 Sync 命名迁移为领域正确的 Project Symlink 命名。当前运行时仍读取 `sync_project_symlink_ignored_dirs` 与 `sync_project_symlink_max_depth`；本 issue 要提供正式数据库迁移，让新旧数据库都能使用同一套 Project Symlink inventory 配置语义。

## Acceptance criteria

- [ ] 新数据库 seed 使用 `project_symlink_ignored_dirs` 与 `project_symlink_max_depth`。
- [ ] 旧数据库升级时，已有 `sync_project_symlink_ignored_dirs` 与 `sync_project_symlink_max_depth` 的用户自定义值被迁移到新 key。
- [ ] 迁移后运行时只读取新 key，不再在 `ProjectSymlinkInventory` 中读取旧 `sync_` key。
- [ ] 对缺失或非法 max depth 值的处理行为与当前实现保持一致。
- [ ] 增加 schema migration 测试，覆盖默认值迁移与用户自定义值保留。
- [ ] 增加 inventory 行为测试，证明迁移后的 ignored dirs 与 max depth 配置仍影响扫描结果。

## Blocked by

None - can start immediately.

## Notes

不要用运行时双读旧 key 作为长期兼容方案；这会让历史命名继续泄露到 inventory 模块。需要兼容旧库时，应通过 schema migration 在启动时完成数据迁移。
