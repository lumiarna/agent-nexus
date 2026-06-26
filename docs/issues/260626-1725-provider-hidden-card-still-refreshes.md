# Provider 隐藏卡片后仍参与 Refresh all / quota polling

## 问题

当前 Provider 页面里的 `Show card on Provider page` 只影响页面展示，不影响 quota query 的创建、自动轮询或 `Refresh all` 的调用范围。

因此，即使某个 Provider 已在页面中隐藏：

- 仍会创建对应的 quota query
- 仍会参与 React Query 的定时刷新
- 点击 `Refresh all` 时仍会再次调用该 Provider 的 quota 接口

这与部分用户对“隐藏后不再刷新”的直觉不一致。

## 现状

### 页面隐藏只影响展示层

```tsx
const visible = ordered.filter((p) => cardVisible[p.id] !== false);
const hidden = ordered.filter((p) => cardVisible[p.id] === false);
```

### quota query 仍按 providerCatalog 全量创建

```tsx
const providerIds = useMemo(
  () => providerCatalog.map((provider) => provider.id),
  [providerCatalog],
);
const quotaResults = useProviderQuotaQueries(providerIds);
```

### Refresh all 仍遍历全部 quotaQueries

```tsx
await Promise.all(
  Object.entries(quotaQueries).map(([id, q]) => {
    // ...
    return q.refetch();
  }),
);
```

### query 启用条件不看 card visibility

```ts
queries: providerIds.map((providerId) => ({
  queryKey: providerKeys.quota(providerId),
  queryFn: () => providersApi.getQuota(providerId),
  enabled: isTauriRuntime(),
  ...providerQuotaRefreshOptions,
}))
```

## 决策待定

需要明确 `Card Visibility` 的产品语义：

### 方案 A：隐藏仅是 surface preference

- 隐藏只影响 Provider 页面卡片是否显示
- 自动轮询继续进行
- `Refresh all` 继续覆盖隐藏项

优点：
- 与 `Card Visibility` 的 glossary 定义更一致
- 切回显示时数据已是热的
- 不把显示偏好混成观测开关

缺点：
- 会继续消耗请求
- 与部分用户直觉不一致

### 方案 B：隐藏后不再参与刷新

- 隐藏项不再自动轮询
- `Refresh all` 跳过隐藏项
- 可能连单卡 refresh 能力也要一起定义清楚

优点：
- 更省请求
- 更符合“隐藏了就别刷新”的直觉

缺点：
- `Card Visibility` 语义从展示偏好扩大成行为控制
- 重新显示时需要重新拉取数据
- 要明确自动轮询 / Refresh all / 单卡 Refresh 三者边界

## 建议

优先考虑一个更保守的中间方案：

- 保持自动轮询行为不变
- 仅让 `Refresh all` 跳过隐藏卡片

这样可以先解决最显式的用户预期冲突，同时避免立即把 `Card Visibility` 变成完整的 quota collection switch。

## 验收标准

- [ ] 明确 `Show card on Provider page` 是否应影响 quota 拉取行为
- [ ] 若影响，则明确影响范围：自动轮询 / `Refresh all` / 单卡 Refresh 分别如何处理
- [ ] 实现后的行为与文案保持一致，避免“只影响本页显示”与实际副作用冲突
- [ ] 添加至少一个测试覆盖隐藏 Provider 在刷新行为上的预期
