- Project 接入 Tauri folder picker，把手输路径替换为原生选择目
- Skill - Project 应显示 Skill 数量
- Icon
- Copy 应先移到回收站再复制
- Local 删除 Copy Task 时，应把目标位置的文件也删掉
- 项目里有三份 Cargo.toml, 两份 package.json/pnpm-workspace.yaml
- 按 ESC 不应退出全屏
- 拖拽排序应保存

---

groups` 用 `nexus.taskGroups()` 做初始值,这本身是一种"兜底假数据"——它让加载中的页面显示**看似真实的假分组**,这才是"卡住/不对劲"的来源。同项目的 `ProjectPage` 用的是更干净的 `query.data ?? []` + 加载态,根本没这个问题。

我这次用 `isLoading` 门控保留了 seed(因为它在无后端的浏览器 demo 态还有用),属于最小改动。但更彻底的做法是让 SyncPage 向 ProjectPage 看齐、删掉 mock-seed。这与你 CLAUDE.md 里"拒绝兜底方案"的原则一致,只是超出了本次三个任务的范围。**

---

6 个 pre-existing clippy warning 未修**：schema.rs 的 `or_else(|x| Err(y))` ×5（v1-v4 旧风格，我的 v5 沿用以保持一致）+ symlink.rs:50 `redundant closure`。属 lint 欠债，超出本 bug 范围。要修建议作为独立 cleanup 任务一次性改 `map_err`。
- **`ROADMAP.md` 有 3 行非本次改动**（`git status` baseline 时 clean，之后被改）。未处理，请你确认是否保留。

---

1. 一个反直觉的实测发现：本机 junction 会命中 `is_symlink()`。** 这有两个含义：① 扫描里我把 junction 判断提到 symlink 之前（否则 junction 被误标 Symlink）；② 你担心的"降级 junction 不被识别"在本机其实**碰巧不成立**（`is_symlink` 认了它），但这是平台/Rust 版本相关的偶然行为，不可依赖——所以我仍改成 `canonicalize` 比较，对 symlink 和 junction 都稳定识别，顺带也确保**已有 symlink 关系不漏**（这正是你最初的顾虑）。结论：B 和 D 在"识别"上殊途同归，B 只是不改 skill 的创建行为。

**2. 一个未来的盲点要先打预防针：prompt 文件分发。** 现在 `create_managed_directory_link` 的降级只对**目录**成立（junction 只能链目录）。skill 是目录，没问题。但若将来 prompt 分发落地且是**单文件**链接，无特权 Windows 上它仍会 symlink 失败、无 junction 可退——届时要么 prompt 也走"复制"，要么接受需开发者模式。现在 prompt 分发后端未实现，不影响，但记在 session 里了。
