# 设置页 Agent Config Root 支持打开路径

## 目标

让用户可从 Settings 的每个 Agent 卡片直接打开其 `CONFIG_ROOT`，无需手动复制路径并在文件管理器中查找。

## 已确认事实

- Settings 的 Agent 卡片由 `src-react/src/components/settings/SettingsPage.tsx` 渲染；`CONFIG_ROOT` 目前仅是不可交互的文本。
- Agent capability surface 已向前端提供 canonical Agent 名称和 `configDir`，由后端 `services/agent_capabilities.rs` 维护。
- 既有 Skill / Prompt 的 Reveal 操作通过 Tauri command 调用 core 的跨平台 `services/system_open` helper，而非前端直接启动外部程序。

## 需求

1. `CONFIG_ROOT` 的路径文本必须可点击。
2. 点击后在操作系统文件管理器中打开对应 Agent 的配置根目录。
3. 打开目标必须由后端根据 canonical Agent 名称解析，不能信任前端传入的任意文件系统路径。
4. `~`、Windows 环境目录与本地路径必须复用 `services::paths::resolve_local_path` 这一唯一解析入口；不得为 Config Root 或 Explorer 新增平行解析器。
5. 打开目录必须复用 `services::system_open::open_path`；目标不存在或不是目录时应明确报错，不得静默退回文件管理器首页。
6. 非桌面运行时或打开失败时，应沿用现有页面的 toast 错误反馈，且不影响 Settings 的其他行为。

## 验收标准

- [ ] Settings 中每个 Agent 的 `CONFIG_ROOT` 具有可点击的交互样式与可理解的 tooltip/辅助说明。
- [ ] 点击 `CONFIG_ROOT` 会通过 typed API 调用专用 Tauri command，并以该 Agent 的 canonical 名称作为输入。
- [ ] Tauri command 从 capability surface 查询 Agent，以共享 `resolve_local_path` 解析 `~` 后调用现有跨平台 `open_path` 打开目录；未知 Agent、目录不存在或目标不是目录时返回明确错误。
- [ ] Windows 下共享 home 解析使用原生 `USERPROFILE`，不会将 Git Bash 风格 `HOME=/c/Users/...` 交给 Explorer；所有路径消费者共用该规则。
- [ ] 浏览器预览（非 Tauri runtime）点击时显示统一的 desktop runtime 错误 toast，不直接尝试启动系统程序。
- [ ] 前端类型检查及相关自动化测试通过。

## 范围外

- 不为 `GLOBAL_SKILLS`、`PROJECT_SKILLS` 或 `GLOBAL_PROMPT` 增加打开目录交互。
- 不修改 Agent capability 配置或 Agent 启用/禁用语义。
- 不引入新的 Tauri 插件或第三方依赖。
