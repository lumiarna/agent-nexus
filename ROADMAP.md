- Windows Rust 测试权限问题导致 TDD 工作流无法闭环
- Windows 支持 Symlink 权限不足时降级为 Junction
- Project 接入 Tauri folder picker，把手输路径替换为原生选择目
- Project 支持拖拽排序
- Icon
- Copy 应先移到回收站再复制

---

顺带一个架构观察（与执行问题正交）**：你的集成测试 `use agent_nexus_lib::{database, services}` 会把整个 `tauri 2.8` 链进测试二进制，而 `#[tauri::command]` 包装层本就独立在 `src/commands/`。把 `database`+`services`+`error` 拆成一个**不依赖 tauri 的 core crate**，能大幅缩短测试编译时间、并让测试在 Windows 原生 / WSL / CI 三处都更轻——这才是让 TDD 长期顺手的根治方向。要不要我评估这个拆分的改动量？
