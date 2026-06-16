# 原型反馈

- 每个一级功能拆分成独立的 HTML 文件
- Provider 需要手动配置两类内容：一类是 quota 观测所需的连接参数，如 OpenCode Go Workspace ID / Auth Cookie；另一类是展示偏好，如卡片隐藏/排序，以及是否显示在 Windows 任务栏。任务栏图标的 `used/remaining` 采用全局统一配置，默认 `remaining`
- Project Key 默认直接用项目文件夹名称，创建时强提醒可手动编辑；首次创建并保存后冻结，不随目录名自动改写
- Agent 需要新增一个 Copilot，作为完整 Agent，在 MVP 中参与 Skill 和 Prompt 的传播；补充 `COPILOT_CONFIG_DIR = ~/.agents`
- Skill 现在每个 item 有点太高了，不好看。Source 路径是冗余信息，可以删除；Open source / Reveal path 按钮可以放到新列，不要单独占一行；传播到不同 Agent 用 Agent 图标表示，不要每个 Agent 单独占一列
- Prompt 的 Source 路径可以保留；Open source / Reveal path 按钮可以放到新列；Agent 也用图标表示
- 现在没有地方配置 WebDAV 的信息，需要补一个配置入口
- Sync 的 Skill Distribution / Prompt Distribution / Session Backup 是系统默认行为，没必要放在最上面那么显眼还占空间，应该放到页面最下面且默认折叠
- Create custom task 的原型也需要设计
- Template 功能应该隐藏在 Create custom task 里面，没有实际 Task 的，不用显示在 Sync 页面上
- 目前 Task List 没有分组的概念，一个 Task Group 可能需要多个 Task（Template 配置的也是 Task Group）
