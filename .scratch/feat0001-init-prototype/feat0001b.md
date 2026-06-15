# 原型反馈 B

- Provider 卡片可拖拽排序
- 示例 Project Key 不要用那么多 github 仓库名，容易造成误解；考虑到 Project Key 如果支持编辑，还要考虑什么时候编辑，所以感觉去掉编辑功能吧，始终使用项目文件夹名称作为 Project Key
- Add Git Base Folder 原型未实现
- Project 详情面的 Skill 显示的列同全局 Skill List
- Project 详情面无需显示 Prompt
- Create custom task 的预设模板往往是 Task Group，需要支持多个 Task
- Distribution 和 Agent config roots，Agent 严格按照 Claude Code / CodeX / Copilot / OpenCode 排序，删除 `New in MVP` 标签
- Backup 和 Distribution 应该是 Task Level 不是 Task Group Level
- 现在没有地方设置 Task 定时器（CRON 表达式）
- Session 页面显示 Cloud 即可，不要 ` (WebDAV)`，事实上 WebDAV 只允许在设置页面出现
- Session 页面的 Project 应挑战到具体 Project Detail，而不是 Project List
- Task Group 之间支持拖拽排序
- Task Group 下的 Task 也支持拖拽排序
