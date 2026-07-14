# 深化 Provider quota surface projection

## Goal

深化现有 `quotaDisplay` module，集中 Provider quota snapshot 到卡片和 Windows 任务栏 surface 的 metric、status 与展示投影规则，让 GUI 壳只承担渲染 adapter 职责。

**推荐强度：Worth exploring**

## Evidence

- `ProviderPage.tsx:119-130,477-491` 独立派生 Used/Remaining 与 pace。
- `src-react/src/components/provider/useTraySync.ts:37-56` 再次解释 failed、missing primary、品牌与 Used/Remaining。
- `src-tauri/src/tray.rs:33-44` 又解释空值失败标记。
- `da7b5ee` 的失败标记修复和 `bf19b64` 的 metric 扩展都跨多个 module 修改。
- `src-react/src/components/provider/quotaDisplay.ts` 已有 depth，适合继续深化而不是创建平行 formatter。

## Requirements

1. quota snapshot、metric 与 surface-specific projection 规则集中到现有深 module。
2. Provider 卡片与 Windows 任务栏可有不同投影，但共同状态解释只定义一次。
3. Tauri Tray 保持图标 reconcile 与像素渲染 adapter，不承担 Provider 领域状态派生。
4. 保持 available、failed、expired、nocreds、missing primary、unlimited、Used/Remaining 的现有可见行为。
5. 不修改 quota 获取 adapter，也不增加新的平台 seam。
6. 实现前比较深化现有 `quotaDisplay` 的不同 interface 形状。

## Acceptance Criteria

- [ ] `useTraySync` 与 ProviderPage 不再各自解释同一 status/metric 规则。
- [ ] 纯测试覆盖所有 snapshot status × Used/Remaining × card/tray surface 的关键组合。
- [ ] Tauri 测试聚焦图标渲染与 reconcile，不复制 Provider quota 领域规则。
- [ ] 失败标记、无 primary 时隐藏、品牌颜色和数值显示保持兼容。
- [ ] deletion test 复核表明删除深化后的 `quotaDisplay` 会让投影复杂度重新扩散到两个 surface。

## Out of Scope

- 修改 DeepSeek 或其他 Provider quota 请求逻辑。
- 改变 Card Visibility 的 polling 语义。
- 新增 GUI runtime adapter。
