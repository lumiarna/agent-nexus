- Project 接入 Tauri folder picker，把手输路径替换为原生选择目
- Skill - Project 应显示 Skill 数量
- Icon
- Copy 应先移到回收站再复制
- Local 删除 Copy Task 时，应把目标位置的文件也删掉
- 项目里有三份 Cargo.toml, 两份 package.json/pnpm-workspace.yaml
- 按 ESC 不应退出全屏
- 列文档表示 Provider 取值顺序、显示的 Credential 是什么
- Claude Code / OpenCode Go 命名
- Sync Task，已经有删除线了，不需要再显示 Missing 字样（和 Project 面对齐）

---

groups` 用 `nexus.taskGroups()` 做初始值,这本身是一种"兜底假数据"——它让加载中的页面显示**看似真实的假分组**,这才是"卡住/不对劲"的来源。同项目的 `ProjectPage` 用的是更干净的 `query.data ?? []` + 加载态,根本没这个问题。

我这次用 `isLoading` 门控保留了 seed(因为它在无后端的浏览器 demo 态还有用),属于最小改动。但更彻底的做法是让 SyncPage 向 ProjectPage 看齐、删掉 mock-seed。这与你 CLAUDE.md 里"拒绝兜底方案"的原则一致,只是超出了本次三个任务的范围。**
