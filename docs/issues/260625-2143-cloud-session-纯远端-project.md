# Cloud Session 支持纯远端 Project

## 问题

当前 Cloud Session 聚合只覆盖本地已收录且 active 的 `Project`。原因是 `session_index.project_id` 非空并外键指向 `projects`，Cloud Session 索引必须归属于一个本地 `Project` 行。

这满足“同一个项目在本机也存在”的 MVP 场景，但不支持以下用户路径：

- 本机尚未收录某个项目，Cloud 中已经有 `Session/<project_key>/` 归档。
- 用户希望先从 Cloud Session 视图发现远端项目，再决定是否恢复、关联或创建本地 Project。
- 多设备共享同一个 WebDAV 归档时，新设备只想浏览历史 Session，而不想先手动添加所有仓库路径。

## Why

`Cloud Session` 在领域模型中是“来自 WebDAV 汇总归档的只读视图聚合”。如果它只能展示本地 `Project` 的远端副本，就会缺失跨设备归档的一个核心价值：在新设备或缺失仓库的设备上发现历史会话。

但直接放开会带来身份与数据模型问题：

- `Project` 当前表示本地已收录 Git repository root，不等于远端归档里的 `project_key`。
- `Project Path` 是本地路径，纯远端项目没有可用路径。
- `session_index.project_id` 当前要求非空外键；纯远端 Session 没有可引用的本地 Project。
- UI 需要区分“本地 Project 的 Cloud Session”和“仅 Cloud 存在的 Project archive”，否则用户会误以为可以打开本地项目。

## What to build

让 Cloud Session 视图能够发现并展示 WebDAV 中存在、但本机尚未收录为本地 `Project` 的远端 Project archive。

完成后，用户在配置好 WebDAV 后，可以从 Cloud Session 看到 `Session/<project_key>/` 下的历史会话，即使本机没有对应仓库路径。纯远端 Project 应表现为只读 Cloud archive；只有当用户显式关联或恢复到本地路径后，才变成普通本地 `Project`。

## Acceptance criteria

- [ ] Cloud scan 能枚举 `Session/` 下的 project key 目录，而不只遍历本地 active projects。
- [ ] 纯远端 Project 的 Cloud Session 能进入索引并在 Session 页 Cloud 来源下展示。
- [ ] 纯远端 Session 能按需读取正文；正文仍不持久化进 SQLite。
- [ ] UI 明确标记纯远端 Project archive，不显示或禁用依赖本地路径的动作，例如 Open Project。
- [ ] 用户可以从纯远端 Project archive 进入一个显式流程，将其关联到已有本地 Git 仓库或创建本地 Project 记录。
- [ ] 本地已收录 Project 与纯远端 Project 使用同一 `project_key` 时不会重复显示为两个独立项目。
- [ ] 迁移保留现有本地 Session 与 Cloud Session 索引，不破坏 `Project` 删除级联语义。
- [ ] 测试覆盖：仅 Cloud 存在的 project key 被发现、Cloud 详情按需 GET、本地 Project 后续出现时能与远端 archive 归并。

## Suggested shape

- 先明确数据模型：不要把“远端 project key”伪装成完整 `Project Path`。可选方向包括：
  - 放松 `session_index.project_id`，补充 `project_key` / `project_name` 字段；
  - 或新增 Cloud archive / remote project projection 表，专门承载没有本地路径的远端项目。
- Cloud scan 先列 `Session/` 目录，再递归列每个 `project_key` 下的 Markdown 文件。
- UI 层以 `project_key` 作为纯远端 archive 的显示与筛选身份；只有本地 Project 存在时才使用 `project_id` 执行本地动作。
- 将“关联到本地 Project”设计为受控流程，而不是自动创建带假路径的 Project。

## Out of scope

- 不做 Cloud→Local 自动 Pull 或恢复正文到本地目录。
- 不做多远端同名 project key 的冲突解决；`project_key` 仍是聚合身份。
- 不引入 manifest 作为聚合真相源；目录仍是 Cloud Session 的物理真相源。
- 不改变 `Project` 的核心定义：本地 `Project` 仍表示已收录 Git repository root。

## Notes

当前实现刻意只覆盖本地已收录 Project，是为了避免在 Session 同步接线阶段顺手重写 `Project` 身份模型。实现本票前应先更新 `CONTEXT.md` 与数据库设计文档，明确纯远端 archive 与本地 Project 的关系。
