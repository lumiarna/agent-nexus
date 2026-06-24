1. 端点根据 OpenCode 配置的 `npm` 字段确定：
   - `@ai-sdk/openai-compatible` → `/chat/completions`
   - `@ai-sdk/openai` → `/responses`
2. Minute/Hourly/Daily/Monthly 标题目前写死映射，但仅展示响应中实际存在的窗口。
3. 明确约定 Monthly 是自然月，按下月 1 日计算剩余时长和预警（UTC时区）。
