# 深化后台运行编排模块

## Goal

深化 Tauri 壳层后台运行编排 module，把作业接入、tick 顺序、错误隔离、分钟对齐与生命周期集中到同一 locality，同时维持 `nexus-core` 的领域独立性。

**推荐强度：Worth exploring**

## Evidence

- `src-tauri/src/lib.rs:24-32` 的 bootstrap 直接 clone 每个后台作业 module。
- `src-tauri/src/lib.rs:138-153` 的 scheduler interface 随作业数量线性增长，并硬编码顺序、错误策略、时钟与休眠。
- `store.rs` 另行构造相同作业依赖。
- `829991d` 新增 Provider Window Alignment 时必须同时修改 store 装配和 scheduler 参数；`e429207` 再次传播依赖。
- `src-tauri` 当前没有分钟边界、错误隔离或循环存活测试。

## Requirements

1. 后台作业接入与 tick 编排集中到壳层 deep module；bootstrap 不学习每个作业的运行细节。
2. 一个作业失败不得阻止同 tick 的其他作业，也不得终止后续 tick。
3. 保持 Sync scheduled task 与 Provider Window Alignment 的现有领域 interface。
4. 不把 Tauri/std runtime 抽成公开 port；当前只有一个生产 adapter，公开 seam 属于假想变化。
5. 如测试需要可控时钟或执行器，只考虑 module 内部 seam。
6. 实现前用 design-it-twice 比较调度表、作业集合等形状，本任务不预定最终 interface。

## Acceptance Criteria

- [ ] `run()` bootstrap 不再逐个 clone 并传递每个后台作业 module。
- [ ] 测试覆盖分钟边界、作业执行顺序、单作业失败隔离和下一轮继续运行。
- [ ] 增加一个测试作业不会要求修改 bootstrap interface。
- [ ] scheduler 不引入 Tauri 依赖到 `nexus-core`。
- [ ] 现有每分钟 Sync 与 Provider Window Alignment 行为保持兼容。
- [ ] deletion test 复核表明删除深化后的 module 会让循环、时钟与错误策略重新扩散到 bootstrap。

## Out of Scope

- 引入通用 workflow engine。
- 新增后台 daemon 或系统服务。
- 为唯一 runtime 创建公开 adapter seam。
