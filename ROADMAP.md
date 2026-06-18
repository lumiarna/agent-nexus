- 当前还保留了 `symlink` 命名；后续可以考虑把这组服务命名逐步改成 `link placement`，因为 junction 现在已经是正式实现之一
- 页面上显示 Link 类型: Symlink / Junction
- Project 接入 Tauri folder picker，把手输路径替换为原生选择目
- Project 支持拖拽排序
- Icon
- Copy 应先移到回收站再复制

---

顺带一个架构观察（与执行问题正交）**：你的集成测试 `use agent_nexus_lib::{database, services}` 会把整个 `tauri 2.8` 链进测试二进制，而 `#[tauri::command]` 包装层本就独立在 `src/commands/`。把 `database`+`services`+`error` 拆成一个**不依赖 tauri 的 core crate**，能大幅缩短测试编译时间、并让测试在 Windows 原生 / WSL / CI 三处都更轻——这才是让 TDD 长期顺手的根治方向。要不要我评估这个拆分的改动量？

---

groups` 用 `nexus.taskGroups()` 做初始值,这本身是一种"兜底假数据"——它让加载中的页面显示**看似真实的假分组**,这才是"卡住/不对劲"的来源。同项目的 `ProjectPage` 用的是更干净的 `query.data ?? []` + 加载态,根本没这个问题。

我这次用 `isLoading` 门控保留了 seed(因为它在无后端的浏览器 demo 态还有用),属于最小改动。但更彻底的做法是让 SyncPage 向 ProjectPage 看齐、删掉 mock-seed。这与你 CLAUDE.md 里"拒绝兜底方案"的原则一致,只是超出了本次三个任务的范围。**

---

6 个 pre-existing clippy warning 未修**：schema.rs 的 `or_else(|x| Err(y))` ×5（v1-v4 旧风格，我的 v5 沿用以保持一致）+ symlink.rs:50 `redundant closure`。属 lint 欠债，超出本 bug 范围。要修建议作为独立 cleanup 任务一次性改 `map_err`。
- **`ROADMAP.md` 有 3 行非本次改动**（`git status` baseline 时 clean，之后被改）。未处理，请你确认是否保留。