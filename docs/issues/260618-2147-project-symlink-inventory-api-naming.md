# Project Symlink Inventory API 命名迁移

## What to build

将 Project Symlink inventory 的外部调用面从 Sync 语义迁移到 Project Symlink inventory 语义。当前 core 和 Tauri state 已拆出 `ProjectSymlinkInventory`，但前端 API/query key 仍挂在 sync 命名下；本 issue 要让用户可见行为不变，同时让代码调用面反映真实领域归属。

## Acceptance criteria

- [ ] 前端不再通过 `syncApi` / `syncKeys` 表达 Project Symlink inventory 查询与删除。
- [ ] Tauri command 注册、前端 invoke 名称、query key 命名使用 Project Symlink inventory 或 Projects 语义，并保持现有页面功能可用。
- [ ] Sync Task 的 API/query 只保留 Task Group、Task execution、WebDAV settings 相关能力。
- [ ] Project Symlink 列表刷新、空态、错误态、删除后 invalidation 行为保持不变。
- [ ] 覆盖至少一个前端或命令层测试，证明删除 Project Symlink 后列表会刷新或重新查询。

## Blocked by

None - can start immediately.

## Notes

优先做兼容迁移：如果 invoke 名称改动会影响已有桌面版本，可以先新增语义正确的新 command，再让旧 command 在一个版本内委托并标记待删除。不要在同一张票里改数据库 settings key。
