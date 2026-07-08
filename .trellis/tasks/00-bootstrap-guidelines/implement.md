# Implementation Plan

## Checklist

1. 读取现有约束
   - `CONTEXT.md`
   - `GOTCHAS.md`
   - `CLAUDE.md`
   - `docs/design/Business Requirement.md`
   - `docs/design/Architecture Design.md`
   - `docs/design/Database Schema.md`
   - `docs/adr/`
2. 盘点当前 `.trellis/spec/` 文件
   - 标记模板内容、重复文件、需要合并或删除的章节。
   - 确认最终 spec 边界与 `index.md` 导航。
3. 分层抽样阅读真实代码与测试
   - `src-react/`：组件、hooks、状态、类型、质量规则。
   - `src-tauri/`：Tauri 命令、core 调用、错误传播、配置。
   - `src-tauri/crates/nexus-core/` 或实际 core crate：数据库、服务、测试、日志。
4. 写入中文 spec
   - 用中文描述规则。
   - 保留术语、路径、命令、代码符号原文。
   - 每条关键规则附真实文件路径或文档依据。
5. 同步索引
   - 更新各层 `index.md`。
   - 删除 `To fill` 状态与脚手架说明。
6. 质量检查
   - 搜索占位内容。
   - 检查链接与路径是否存在。
   - 检查 spec 是否互相矛盾。

## Validation Commands

```bash
grep -R "To fill\|TODO: fill\|placeholder\|TBD" .trellis/spec
find .trellis/spec -name "*.md" -maxdepth 4 | sort
```

如涉及 Rust 验证说明，遵守 `GOTCHAS.md`：Windows 下不要直接使用裸 `cargo test -p nexus-core`，优先记录 `pnpm rust:test` 或 `node scripts/with-sqlite.mjs cargo test -p nexus-core`。

## Risky Areas

- 领域术语翻译：必须遵守 `CONTEXT.md`，避免把 `Agent` 误写为 provider/account/model。
- Backend 边界：区分 Tauri shell 与 `nexus-core`，不要混写职责。
- 验证命令：避免写入在 Windows SQLite 环境下已知会失败的裸命令。
- 模板残留：现有 spec 多处明确要求英文或待填充，必须改掉。

## Before Start Gate

- `prd.md`、`design.md`、`implement.md` 已完成。
- `implement.jsonl` 与 `check.jsonl` 已包含真实上下文条目。
- 用户确认可进入实现阶段后，再运行 `task.py start` 或等效 Trellis start 流程。