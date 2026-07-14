# 深化 Provider Display Preferences 模块

## Goal

让 Provider Display Preferences 的领域所有权、默认值语义、catalog 合并、持久化与失败回滚集中到更深 module，消除“未保存”与“明确保存为空”的歧义。

**推荐强度：Strong**

## Evidence

- `src-react/src/components/provider/useProviderDisplayPrefs.ts:46-225` 同时承担 catalog 合并、DND、三种偏好更新、完整记录重建与回滚。
- 当前以 `savedVisible.size > 0` 判断是否保存过，使“隐藏全部卡片”和“从未保存”共享空数组表示。
- 前端测试复制 helper 实现，而不是通过生产 module interface 测试。
- `docs/issues/260624-1509-provider-display-preferences-return-to-provider-domain.md` 已记录 display preferences 被拆到通用 settings 与 `providers` 表之间的领域漂移。
- `8349002`、`3efd9b3`、`9e4d28c`、`e2ba9f3` 连续修改该路径。

## Requirements

1. `Provider Display Preferences`、`Surface Preference` 与 `Card Visibility` 使用 `CONTEXT.md` 领域语义。
2. Card Visibility、Tray Visibility 仍是不同 surface 的偏好；不能因共同持久化而混为一个含义。
3. `Tray Metric Mode` 保持全局偏好，不错误归属到单个 Provider。
4. 持久化必须区分“尚未显式设置”与“显式空集合”。
5. Provider 排序、Card Visibility 与 Tray Visibility 应通过 Provider-facing module 读取和写入，而不是由页面拼接通用 settings。
6. 新发现 Provider 的默认合并规则必须确定且可测试。
7. 实现前通过 grilling 处理现有数据迁移与 interface 选择，本任务当前不预定最终 interface。

## Acceptance Criteria

- [ ] 用户隐藏全部 Provider 卡片后，重启仍保持全部隐藏。
- [ ] 未保存偏好时仍按 catalog 默认值初始化。
- [ ] catalog 新增 Provider、重复 ID、旧偏好缺项均有确定合并结果。
- [ ] 排序、Card Visibility、Tray Visibility 的成功与失败回滚通过生产 module interface 测试。
- [ ] Provider 页面不再自行重建完整偏好 payload 或合并多个领域真相源。
- [ ] 既有偏好数据有兼容或低成本迁移路径，符合未上线阶段约束。
- [ ] deletion test 复核表明删除深化后的 module 会让默认值、合并和回滚规则扩散回 ProviderPage。

## Out of Scope

- 改变 Card Visibility 是否控制 quota polling；该产品语义仍由 `docs/issues/260626-1725-provider-hidden-card-still-refreshes.md` 单独决定。
- 重做 Provider 卡片布局。
- 修改 Provider quota adapter。
