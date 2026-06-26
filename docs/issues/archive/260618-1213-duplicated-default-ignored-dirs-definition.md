# 默认忽略表重复定义

## 问题

`sync_project_symlink_ignored_dirs` 的默认值在两处独立维护，靠人工同步，无自动校验：

- `src-tauri/src/services/sync.rs` 第 25 行：`DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &[&str]` 数组，运行时 `project_symlink_ignored_dirs()` 在 DB 无值时使用。
- `src-tauri/src/database/schema.rs` 第 8 行：`NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &str` 多行字符串，schema v1 seed 和 v5 migration 写入 DB 时使用。

两份内容必须一致，但分属不同模块、不同类型（`&[&str]` vs `&str` 换行分隔），改一处忘改另一处会产生静默分歧：新库 seed 用新值、运行时 fallback 用旧值（或反之），导致行为取决于「库是新建还是迁移来的」。

## 修复建议

两选一：

1. **单一来源**：让 `sync.rs` 的 `project_symlink_ignored_dirs()` fallback 直接读 schema 模块导出的常量（解析换行字符串为 `HashSet`），删除 `&[&str]` 数组。schema 是默认值的权威来源，运行时只消费。
2. **加一致性测试**：保留两份定义，在 `schema.rs` 或 `sync.rs` 的 `#[cfg(test)]` 加测试，断言 `NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS` 解析后的集合 == `DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS.iter().collect()`。锁定一致性，分歧时测试红。

方案 1 更彻底（消除重复），方案 2 改动更小（加测试即可）。建议优先方案 1。

## 备注

同样的问题存在于 `sync_project_symlink_max_depth`：`sync.rs` 的 `DEFAULT_PROJECT_SYMLINK_MAX_DEPTH: usize = 3` 与 `schema.rs` 的 `DEFAULT_PROJECT_SYMLINK_MAX_DEPTH: &str = "3"`，但 `usize` vs `&str` 差异更小、单值更难分歧，风险低于忽略表。
