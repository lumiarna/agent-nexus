# 架构深化审查结果

日期：2026-07-14

## Scope

审查由近 60 次提交热点驱动，重点覆盖：

- Project / Skill / Prompt / Distribution
- Sync Task Group / Sync Task
- Provider quota / Display Preferences / Window Alignment / Windows 任务栏
- Tauri 壳层 bootstrap、后台运行与窗口生命周期

已读取 `CONTEXT.md`、`GOTCHAS.md`、`docs/design/*`、ADR-0001～0003、相关 specs、测试、历史 issue 与过去架构审查对话。过去审查已经落地的 Distribution 基础、Placement ownership、Provider quota ports、Task lifecycle、cron/Transfer、taskRules 与 React Query 单一真相源不重复包装为新候选；只有出现新摩擦的增量部分被保留。

## Candidate Matrix

| Priority | Strength | Candidate | Evidence summary | Child task |
|---|---|---|---|---|
| 1 | Strong | Project custom Skill 传播 module | 连续四次功能/修复；canonical/projection 身份与编排泄漏到两个页面 | `07-14-deepen-project-custom-skill-propagation` |
| 2 | Strong | Distribution source relocation | Skill/Prompt 两个真实 adapter 重复 relocation 与逆序补偿状态机 | `07-14-deepen-distribution-source-relocation` |
| 3 | Strong | Sync Task Group cache mutation module | 排序、重命名、折叠三次变化重复穿透页面与 query module | `07-14-deepen-sync-task-group-cache-mutations` |
| 4 | Strong | 用户 Task Group 持久化 module | 用户/系统托管写权限以不同 SQL 形式散落，部分写路径可越过不变量 | `07-14-deepen-user-task-group-persistence` |
| 5 | Strong | Provider Window Alignment capability 真相源 | 前端先用硬编码名单决定是否查询后端 capability，新增 adapter 要改两层 | `07-14-provider-window-alignment-capability-source` |
| 6 | Strong | Provider Display Preferences module | 未设置与显式空集合冲突；Provider 状态分裂在 settings 与页面拼装 | `07-14-deepen-provider-display-preferences` |
| 7 | Worth exploring | Provider quota surface projection | Card 与 Tray 重复解释 status/metric，失败标记修复跨层传播 | `07-14-deepen-provider-quota-surface-projection` |
| 8 | Worth exploring | 后台运行编排 module | scheduler interface 随作业线性增长，时钟与错误策略无测试面 | `07-14-deepen-background-runner-orchestration` |
| 9 | Worth exploring | Tray / Window 生命周期 module | Close/Hide/Show/Quit 策略横跨 `lib.rs` 与 `tray.rs` | `07-14-deepen-tray-window-lifecycle` |

## Top Recommendation

### Project custom Skill 传播 module

这是近期最热且修复链最长的路径。`df9306a` 引入跨 Project propagation，`db018a7` 补 source Project 回传，`53bd3e5` 修 projection row 的 Global 可见性，`997ca77` 再修 propagated symlink 泄漏。当前调用者仍需理解 `canonicalSkillId ?? id`、`placementProjectId`、canonical/projection row 与不同 mutation 返回形状。

Deletion test：删除当前 `propagation.ts` 只会让少量数组计算回到页面，说明它 shallow；若删除深化后的 module，身份解析、传播状态和调用编排会重新扩散到 SkillPage、ProjectDetailView、SkillRow、query hooks 与 Rust projection 逻辑，因此该候选能产生真实 depth、locality 与 leverage。

## Candidate Notes

### Distribution source relocation

现有 Distribution module 已 deep，但 relocation 是后来新增且未被吸收的实现。Skill 与 Prompt 已是两个真实 adapter，适合深化已有 seam；不得违反 ADR-0003 去强行统一目录/文件差异。

### Sync Task Group cache mutation

`lib/query/sync.ts` 隐藏了一半 cache 协议，`SyncPage.tsx` 又承担另一半 snapshot / optimistic update / rollback。Deletion test 表明 query module 应保留并增加 depth，而不是删除或再套一层 pass-through。

### 用户 Task Group 持久化

`TaskLifecycle` 整体已有 depth；候选是内部深化，而非推翻架构文档或抽新 orchestration seam。尤其需要把 Session Backup 可 Run/可 Schedule、不可新增/删除/排序的领域约束集中到 core interface。

### Provider Window Alignment capability

删除前端 `supportsWindowAlignment` 后复杂度反而消失，证明它是浅重复。后端已有 Claude Code、CodeX 与测试 runner 多个 adapter，是真实 seam。

### Provider Display Preferences

本候选吸收现有 `docs/issues/260624-1509-provider-display-preferences-return-to-provider-domain.md` 的领域所有权问题，并新增明确证据：空 `cardVisibility` 无法区分“未保存”与“隐藏全部”。Card Visibility 是否影响 polling 仍属于另一个产品决策，不混入本任务。

### Provider quota surface projection

现有 `quotaDisplay` 已通过 deletion test；应继续深化，而不是另建 formatter。Tauri Tray 只保留渲染 adapter。

### 后台运行编排

当前只有 Tauri/std 一个生产 runtime adapter，不能为了测试建立公开假想 seam。可控时钟若必要，只作为 module 内部 seam。

### Tray / Window 生命周期

现有 Tray module 已 deep，但生命周期策略泄漏到 bootstrap。当前只有 Tauri GUI adapter，不建立公开 GUI runtime seam。

## Rejected Candidates

1. **统一 Skill / Prompt / Session custom source**：直接违背 ADR-0003；物理与领域差异是真实差异，不是重复。
2. **继续拆 Tauri command 注册表**：宏清单是机械 adapter；删除只会搬家，无法集中复杂度。
3. **为 AppState 建新 seam**：当前是合理 composition root，只有一个生产装配 adapter。
4. **重做 Provider quota core**：已有真实 credential/transport seam 与 fake adapter，当前 depth 足够。
5. **重做 Transfer seam**：已有 WebDAV 与 RecordingTransfer 两个 adapter，现有测试面稳定。
6. **把 `with-sqlite` PowerShell 改写为 Node**：已有 issue 明确“暂不做”，且不是近期产品代码热点。

## Cross-task Ordering

- Top recommendation 可独立先做。
- Distribution source relocation 与 Project custom Skill propagation 均会修改 `skills.rs`，建议串行，优先 Project custom Skill，随后 relocation，减少冲突。
- Sync 前端 cache mutation 与 core Task Group persistence 可并行规划，但实施时建议先完成 core 不变量，再验证前端权威响应。
- Provider capability、Display Preferences、quota projection 领域相邻但可独立；若并行实施需避免同时大改 `ProviderPage.tsx`。
- 后台运行编排与 Tray / Window 生命周期都修改 `src-tauri/src/lib.rs`，建议串行。
