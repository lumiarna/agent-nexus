# Agent Nexus

## Hero

- 标题：`A Nexus for Shared Agent Assets`
- 副标题：`Your agents may be different, but the assets they rely on do not have to be. Manage one shared layer for providers, projects, skills, prompts, and sessions.`

## 基本要求

- 跨平台：macOS/Windows
- Theme: Light/Dark
	- 默认 Light
- 无需支持多语言，页面元素默认英文（用户资产可能包含中文或其他语言）

## Agents

- Generic Agent
	- `${GENERIC_AGENT_CONFIG_DIR}`: `~/.agents`
	- `${GENERIC_AGENT_SKILLS_DIR}`: `${GENERIC_AGENT_CONFIG_DIR}/skills`
	- `${GENERIC_AGENT_PROMPT_FILE}`: `${GENERIC_AGENT_CONFIG_DIR}/AGENTS.md`
- Claude Code
	- `${CLAUDE_CONFIG_DIR}`: `~/.claude`
	- `${CLAUDE_SKILLS_DIR}`: `${CLAUDE_CONFIG_DIR}/skills`
	- `${CLAUDE_PROMPT_FILE}`: `${CLAUDE_CONFIG_DIR}/CLAUDE.md`
- CodeX
	- `${CODEX_CONFIG_DIR}`: `~/.codex`
	- `${CODEX_AUTH_FILE}`：`${CODEX_CONFIG_DIR}/auth.json`
	- `${CODEX_SKILLS_DIR}`: `${CODEX_CONFIG_DIR}/skills`
	- `${CODEX_PROMPT_FILE}`: `${CODEX_CONFIG_DIR}/AGENTS.md`
- Copilot
	- `${COPILOT_CONFIG_DIR}`: `~/.github`
	- `${COPILOT_SKILLS_DIR}`: `${COPILOT_CONFIG_DIR}/skills`
	- `${COPILOT_PROMPT_FILE}`: `${COPILOT_CONFIG_DIR}/AGENTS.md`
- OpenCode
	- `${OPENCODE_CONFIG_DIR}`: `~/.config/opencode`
	- `${OPENCODE_AUTH_FILE}`：`~/.local/share/opencode/auth.json`
	- `${OPENCODE_SKILLS_DIR}`: `${OPENCODE_CONFIG_DIR}/skills`
	- `${OPENCODE_PROMPT_FILE}`: `${OPENCODE_CONFIG_DIR}/AGENTS.md`

## 资产对象

### Provider

- Claude Code
	- Quota 数据来源：`${CLAUDE_CONFIG_DIR}`
- CodeX
	- Quota 数据来源：`${CODEX_CONFIG_DIR}`
- Copilot
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`（`github-copilot.access` 或 `github-copilot.key`）；用户可在 Settings 填 `COPILOT_GITHUB_TOKEN` 优先使用
- OpenCode Go
	- Quota 数据来源：手动填写 `opencode_workspace_id` & `opencode_auth_cookie`
- MiniMax Token Plan CN
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`
- DeepSeek
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`
- OpenRouter
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`
- 通过 `${OPENCODE_CONFIG_DIR}` 自动扫描出的其他 Provider

### Project

- Project = 一个被系统收录的 Git 仓库根目录
- 支持添加 Git Base Folders，自动搜索 Git 项目，如
	- `${ROOT}/Vault`
	- `${ROOT}/Workspace`
	- `${ROOT}/Sample`
- 也支持手动添加项目路径
- 无需支持递归扫描

### Skill

- 来源（排除 Symlink）
	- 自动扫描每种 Agent Global SKILLS_DIR
	- 自动扫描每种 Agent 每个 Project 下的 SKILLS_DIR
	- 支持手动添加
- 显示 name/description
- 支持切换 `disable-model-invocation`
- 支持 Symlink 到其他 Agent
	- 需排除当前来源 Agent
	- Global 的 Symlink 到 Global，Project 的 Symlink 到 Project
	- 手动添加的可以不支持 Symlink，如有需求，手动在 Symlink & Copy 管理里添加
- 支持快捷打开文件所在位置
- 支持 WebDAV 备份

#### Project Skill Direction

- Generic Agent: `${project_dir}/.agents/skills`
- Claude Code: `${project_dir}/.claude/skills`
- CodeX: `${project_dir}/.codex/skills`
- Copilot: `${project_dir}/.github/skills`
- OpenCode: `${project_dir}/.opencode/skills`

### Prompt

- 来源（排除 Symlink）
	- 自动扫描每种 Agent Global PROMPT_FILE
	- 自动扫描每个 Project 的仓库根提示文件
		- Generic Agent: `${project_dir}/AGENTS.md`
		- Claude Code: `${project_dir}/CLAUDE.md`
- Project Prompt 名称使用「项目名 · 文件」，Global Prompt 直接使用文件名
- 支持按文件正文搜索
- 支持 Symlink 到其他 Agent
	- Global Prompt 支持全部 prompt-capable Agent
	- Project Prompt 仅支持 Generic Agent 与 Claude Code
- 支持 WebDAV 备份

## Sync

- Sync Group = one source -> many targets
- Source Type
	- 单文件
	- 单目录
	- 受规则扫描得到的文件集合/目录集合
- Action
	- Symlink
	- Copy
- Target Type
	- Local
	- WebDAV
- target 不可提升为 source
- 如需反向同步，必须新建另一个 Sync Group，而不是在原组内回流
- 以及手动删除和自动扫描失效 Source，Copy 时支持手动同步和自动同步（CRON 表达式）

### 场景案例

- 场景 1：
	- 创建分组 TAP
	- 将 `D:/Workspace/tap/backend/` Symlink 至 `D:/Workspace/oll-context/backend/`
	- 将 `D:/Workspace/tap/frontend/` Symlink 至 `D:/Workspace/oll-context/frontend/`
- 场景 2
	- 创建分组 TAP Builder Skill
	- 将 `D:/Workspace/oll-context/.github/skills/tap-builder/` Symlink 至 `D:/Workspace/tap-kit/.agents/skills/tap-builder/`
	- 将 `D:/Workspace/oll-context/.github/skills/tap-builder/` Copy 至 `D:/Workspace/awesome-vibe-coding/.agents/skills/tap-builder/`，勾选自动同步
- 场景 3
	- 创建分组 SSH
	- 将 .ssh 文件 Copy 至 WebDAV，达成备份效果
- 填写路径的时候最好全都规范化为 `/` 而非 `\`

### Session

- 对于每个 Project，生成系统托管的 Session Backup Copy Task
  - Source：`Local {{project_dir}}/__sessions/`
  - Target：`Cloud Session/{{project_key}}/`
  - Schedule 默认值：`0 * * * *`，允许逐 Task 调整
- Session Backup 在 System-managed records 中复用 Task Group UI，支持 Run、Run Group、Schedule；不支持修改 Source/Target/Action、新增、删除或拖拽排序
- `{{project_dir}}` 展开为当前设备的 Project Path；`{{project_key}}` 展开为跨设备稳定的 Project Key
- 设置 Session 目录，一般是设置 WebDAV 的本地目录
- 支持汇总展示和搜索 WebDAV 同步后的 Session

### 软件配置同步

- 借助 WebDAV 和 Copy 管理完成，只支持同平台互相同步即可
- 可以认为就是一批 Symlink & Copy 管理的预设

#### Zed

- `%APPDATA%/Zed/settings.json`
- `%APPDATA%/Zed/keymap.json`

#### Warp

- `${LOCALAPPDATA}/warp/Warp/config/settings.toml`
- `${LOCALAPPDATA}/warp/Warp/config/keybindings.yaml`

## 其他功能

### Provider Quota Monitor

- Windows 任务栏显示 Provider 剩余额度（agent-quota-monitor 的主要功能）

### Provider 额度定时重置

- 可设置每天定时如 `05:00`/`10:00`/`15:00`/`20:00` 自动触发对话
- 每个 Provider 单独设置

### WebDAV

- 备份 Agent Nexus 的配置
