# 架构深化机会审查 — 执行计划

## Completed Review Steps

- [x] 读取 `CONTEXT.md`、`GOTCHAS.md`、设计文档与 ADR。
- [x] 统计近期提交与文件热点。
- [x] 检索过去架构审查和现存 issue，排除已落地或已明确拒绝的候选。
- [x] 分别审查资产传播、Sync、Provider 与 Tauri 壳层。
- [x] 对候选执行 deletion test、seam/adapter 检查和测试面评估。
- [x] 创建九个独立 Trellis 子任务并写入初始 `prd.md`。
- [x] 记录推荐强度、Top recommendation、拒绝项与跨任务顺序。

## Follow-up Gate Per Child

1. 选择一个子任务，不批量启动全部任务。
2. 运行 grilling，确认约束与风险。
3. 若需要新 interface，按 design-it-twice 生成至少两种方案并比较 depth、locality、leverage 与 seam placement。
4. 完成子任务 PRD convergence pass。
5. 为复杂任务补齐 `design.md`、`implement.md`、`implement.jsonl` 与 `check.jsonl`。
6. 用户审阅后才运行 `task.py start`。
7. 实施、质量检查、spec 更新、提交与归档均在子任务内独立完成。

## Validation

```bash
python3 ./.trellis/scripts/task.py list --status planning
python3 ./.trellis/scripts/task.py validate 07-14-architecture-deepening-review
for task in \
  07-14-deepen-project-custom-skill-propagation \
  07-14-deepen-distribution-source-relocation \
  07-14-deepen-sync-task-group-cache-mutations \
  07-14-deepen-user-task-group-persistence \
  07-14-provider-window-alignment-capability-source \
  07-14-deepen-provider-display-preferences \
  07-14-deepen-provider-quota-surface-projection \
  07-14-deepen-background-runner-orchestration \
  07-14-deepen-tray-window-lifecycle; do
  python3 ./.trellis/scripts/task.py validate "$task"
done

git diff --check
```

## Rollback Points

- 子任务之间没有隐式依赖；错误候选可单独 unlink / 删除。
- Provider 与 Sync 的相邻任务实施时避免并行修改同一大页面。
- 本父任务不启动产品实现，因此当前回滚只涉及 `.trellis/tasks/` 规划工件。
