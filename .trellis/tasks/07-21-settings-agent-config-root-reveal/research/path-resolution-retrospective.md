# 路径解析重复踩坑复盘

## 1. 根因分类

- **E — 隐式假设**：共享 `paths::home_dir` 在所有平台都优先读取 `HOME`，隐含假设其总是原生绝对路径；Windows 从 Git Bash/Orca 等父进程启动时可能得到 `/c/Users/...`，原生 Explorer 无法消费。
- **D — 测试覆盖缺口**：初始测试只覆盖“临时 HOME 能展开”，没有覆盖 Windows 同时存在 `HOME=/c/...` 与 `USERPROFILE=C:\...` 的真实进程环境，也没有验证无效目标必须在启动 Explorer 前失败。
- **复用失守**：第二次修复新增 `resolve_local_filesystem_path`，把共享解析器的缺陷局部绕开，形成两个 home/path 语义入口。

## 2. 前几次修复为什么失败

1. **直接把 capability 的 `~/.pi/agent` 交给 `resolve_local_path`**：调用链复用了正确模块，但共享 `home_dir` 的 Windows 优先级本身错误，仍解析成 Git Bash 路径。
2. **新增 `resolve_local_filesystem_path`**：修复了当前命令的局部输入，却让 Provider、Project、Sync、Skill/Prompt 等既有消费者继续走旧语义；属于症状修复。
3. **从 `reveal_path` 改为 `open_path`**：目录应直接打开这一 UX 判断合理，但若解析出的路径无效，Explorer 仍可能退回首页；没有消除路径真相源问题，也缺少存在性保护。

## 3. 最终架构

- 唯一 home 真相源：`services::paths::home_dir`。
  - Windows：`USERPROFILE` 优先，`HOME` fallback。
  - 非 Windows：`HOME`。
- 唯一路径展开入口：`services::paths::resolve_local_path`。
- 唯一展示折叠入口：`services::paths::collapse_home`。
- 唯一系统打开边界：`services::system_open::{open_path, reveal_path}`。
- `open_path` / `reveal_path` 在启动系统 handler 前统一拒绝不存在目标；领域 service 继续负责“Config Root 必须是目录”等额外语义。
- 前端只传 canonical `AgentName`，不传路径。

## 4. 防复发机制

| 优先级 | 机制 | 动作 | 状态 |
|---|---|---|---|
| P0 | 架构 | 删除功能专用 resolver，统一修复共享 `home_dir` / `resolve_local_path` | DONE |
| P0 | 测试 | 覆盖 Git Bash HOME + Windows USERPROFILE 组合 | DONE |
| P0 | 运行时 | Open / Reveal 对缺失目标 fail-fast，不允许静默落到文件管理器首页 | DONE |
| P1 | 测试隔离 | HOME/USERPROFILE 测试同时保存恢复，并串行化同进程 reader/writer | DONE |
| P1 | 规范 | 在 backend quality spec 与 code-reuse guide 写入唯一解析器/唯一 opener 契约 | DONE |

## 5. 系统性扩展

- Claude Code / CodeX credential path display 原先直接读取 `HOME` 并手写 `strip_prefix`，已改为共享 `path_to_string + collapse_home`。
- Project、Sync、Provider 测试中的 home fixture 已同步设置 `HOME` 与 `USERPROFILE`，避免 Windows 平台与并发测试污染。
- 后续任何本地路径功能都应先检索 `paths.rs` / `system_open.rs`，不得在 command 或 provider 内新增环境变量路径转换。
