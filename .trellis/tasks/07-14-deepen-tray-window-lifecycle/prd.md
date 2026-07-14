# 深化 Tray 与主窗口生命周期模块

## Goal

深化现有 Tray module，把 Close、Hide、Show、Quit、图标存在性与主窗口恢复策略集中到一个可验证 interface，提高 Tauri 壳层 locality。

**推荐强度：Worth exploring**

## Evidence

- `src-tauri/src/lib.rs:117-131` 根据 Tray 图标状态决定 Close 后 hide 或 exit。
- `src-tauri/src/tray.rs:49-153` 分别维护图标状态、reconcile、菜单与窗口恢复。
- 理解“关闭窗口后是否继续运行”需要在 bootstrap 事件处理和 Tray module 间跳转。
- `e2ba9f3` 引入 Tray 时跨多个壳层文件修改；`da7b5ee` 随后修复失败图标行为。
- 现有测试只覆盖纯渲染 helper，未覆盖生命周期行为。

## Requirements

1. Close、Hide、Show、Quit 和 Tray 图标状态的策略集中到现有 Tray/Window 协作 module。
2. 有可恢复 Tray 入口时 Close 隐藏主窗口；无入口时 Close 退出，保持现有用户行为。
3. Tray 点击或菜单恢复窗口时继续执行 show、unminimize 与 focus。
4. bootstrap 只通过较小 interface 委托生命周期行为。
5. 当前只有 Tauri GUI adapter，不创建公开 GUI runtime seam；如需测试隔离，只使用私有内部 seam。
6. 实现前比较至少两种 interface 形状，本任务当前不预定最终 interface。

## Acceptance Criteria

- [ ] `lib.rs` 不再自行解释 `TrayManager::has_icons()` 来决定窗口生命周期。
- [ ] 测试覆盖有图标 Close→hide、无图标 Close→exit、Tray 恢复窗口及显式 Quit。
- [ ] reconcile 与 Close 相邻发生时具有确定、可测试行为。
- [ ] 现有图标渲染、Provider quota 更新和菜单行为保持兼容。
- [ ] 不新增第二套 Tray 状态真相源。
- [ ] deletion test 复核表明删除深化后的 module 会让生命周期策略重新扩散到 bootstrap 与 command adapter。

## Out of Scope

- 重写 Tray 图标视觉。
- 新增其他 GUI runtime adapter。
- 修改 Provider quota 领域规则。
