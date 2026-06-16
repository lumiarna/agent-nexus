# Agent Nexus

## Hero

- 标题：`A Nexus for Shared Agent Assets`
- 副标题：`Your agents may be different, but the assets they rely on do not have to be. Manage one shared layer for skills, prompts, sessions, projects, and quotas.`

## 基本要求

- 跨平台：MacOS/Windows
- Theme: Light/Dark
	- 默认 Light
- 无需支持多语言，页面元素默认英文（用户资产可能包含中文或其他语言）

## Agents

- Claude Code
	- `${CLAUDE_CODE_CONFIG_DIR}`: `~/.claude`
	- `${CLAUDE_CODE_SKILLS_DIR}`: `${CLAUDE_CODE_CONFIG_DIR}/skills`
	- `${CLAUDE_CODE_PROMPT_FILE}`: `${CLAUDE_CODE_CONFIG_DIR}/CLAUDE.md`
- CodeX
	- `${CODEX_CONFIG_DIR}`: `~/.codex`
	- `${CODEX_AUTH_FILE}`：`${CODEX_CONFIG_DIR}/auth.json`
	- `${CODEX_SKILLS_DIR}`: `${CODEX_CONFIG_DIR}/skills`
	- `${CODEX_PROMPT_FILE}`: `${CODEX_CONFIG_DIR}/AGENTS.md`
- Copilot
	- `${COPILOT_CONFIG_DIR}`: `~/.agents`
	- `${COPILOT_SKILLS_DIR}`: `${COPILOT_CONFIG_DIR}/skills`
	- `${COPILOT_PROMPT_FILE}`: `${COPILOT_CONFIG_DIR}/AGENTS.md`
- OpenCode
	- `${OPENCODE_CONFIG_DIR}`: `~/.config/opencode`
	- `${OPENCODE_AUTH_FILE}`：`~/.local/share/opencode/auth.json`
	- `${OPENCODE_SKILLS_DIR}`: `${OPENCODE_CONFIG_DIR}/skills`
	- `${OPENCODE_PROMPT_FILE}`: `${OPENCODE_CONFIG_DIR}/AGENTS.md`

## 资产对象

### Provider

- ? 具体逻辑参考代码，可能有出入

---

- Claude Code
	- Quota 数据来源：`${CLAUDE_CODE_CONFIG_DIR}`
- CodeX
	- Quota 数据来源：`${CODEX_CONFIG_DIR}`
- Copilot
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`/`${GITHUB_TOKEN}`/`${GH_TOKEN}`
- OpenCode Go
	- Quota 数据来源：手动填写 `opencode_workspace_id` & `opencode_auth_cookie`
- MiniMax CN
	- Quota 数据来源：`${OPENCODE_AUTH_FILE}`
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

### Prompt

- 来源（排除 Symlink）
	- 自动扫描每种 Agent Global PROMPT_FILE
- 支持 Symlink 到其他 Agent
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

- 对于每个 Project，扫描 `${project_dir}/__sessions`，通过 WebDAV 同步
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
