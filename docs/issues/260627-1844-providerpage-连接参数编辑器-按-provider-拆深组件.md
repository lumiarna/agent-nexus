# ProviderPage 连接参数编辑器按 Provider 拆深组件

> 架构深化 issue（improve-codebase-architecture，前端）。推荐强度：**Worth exploring**。
> 词汇遵循 `CONTEXT.md`（Provider / Provider Connection Params / Provider Display Preferences）与 codebase-design（module / interface / depth / locality）。

## 问题

`src-react/src/components/provider/ProviderPage.tsx`（1149 行）单个 `ProviderPage` 组件约 930 行、**20 个 useState**。其中一个 config modal（`configId`，242 行）把多个 Provider 各自的 `Provider Connection Params` 揉进一个 modal，靠对 `configId` 取值**硬分支**：

| provider 分支 | 专属 state（行号） | 专属 effect（行号） |
| --- | --- | --- |
| `configId === "copilot"` | `copilotToken` / `copilotTokenSaving` `243-244` | `336-349` |
| `configId === "opencode-go"` | `opencodeGoWorkspaceId` / `opencodeGoAuthCookie` / `opencodeGoSaving` `245-247` | `352-369` |
| `isApiKeyProvider(configId)` | `providerApiKey` / `providerApiKeySaving` `248-249` | `372-385` |
| `configId === "claude"` | window-alignment 一组 `250-254` | `388-392` |

每接一个需要连接参数的 Provider，就往这个 modal 加一组 state + 一个 effect + 一段分支——和后端 `[[260627-1838-provider-quota-按-provider-垂直切分]]` 是**同构的 friction**：一个 Provider 的连接参数知识被横切摊在 modal 各处，无 per-provider locality。

此外 display-preferences 一组（`order` / `cardVisible` / `trayVisible` / `colCount` / `trayMetric`）对应 `CONTEXT.md` 的 `Provider Display Preferences`，与后端 `[[260624-1509-provider-display-preferences-return-to-provider-domain]]` 是同一概念在前端的散落态。

### deletion test

把 OpenCode Go 的连接参数编辑抽成 `<OpenCodeGoConnectionForm>` 后删掉它 → workspaceId/authCookie 的 state、effect、保存逻辑会散回 ProviderPage 顶层 ⇒ 复杂度集中，是真组件。

## What to build

### 1. 每个需要连接参数的 Provider 一个 `ConnectionParamsForm`（深组件）

modal 只负责：选中 Provider → 渲染该 Provider 对应的 form。form 是小 props 的深组件：

```tsx
type ConnectionParamsFormProps<T> = {
  providerId: string;
  initial: T;
  onSave: (value: T) => Promise<void>;   // 注入对应 mutateAsync
};

// 实例：<CopilotTokenForm /> / <OpenCodeGoConnectionForm /> / <ApiKeyForm />
```

- modal 内不再有 `configId === "copilot" ? ... : configId === "opencode-go" ? ...` 的分支墙；改为一张 `providerId → Form` 的注册表，与后端 `provider_quota_adapters()` 注册表呼应。
- 每个 form 自管自己的输入态与保存态（吃掉 `copilotToken`/`opencodeGoWorkspaceId`/`providerApiKey` 等顶层 state 与各自的 saving flag）。

### 2. `useProviderDisplayPrefs` hook

把 `order` / `cardVisible` / `trayVisible` / `colCount` / `trayMetric` 及其持久化收敛进一个 hook，return 小 interface。与后端 `[[260624-1509-provider-display-preferences-return-to-provider-domain]]` 对齐：前端从一个 Provider 偏好钩子读写，而非在页面顶层拼装多个 record state。

## Suggested shape

- **form 接受依赖、可单测**：`onSave` 由 ProviderPage 用既有 `lib/api/providers.ts` / `lib/query/providers.ts` 的 mutation 注入；form 本身无 Tauri 依赖，可注入 fake `onSave` 单测「填写 → 保存 → 清空/saving 态」。
- **per-provider 文件**：放 `components/provider/connection/`，每个 form 一文件，新增 Provider = 新增一个 form 文件 + 注册表一行。
- **window-alignment 单独看**：`configId === "claude"` 的窗口对齐调度（cron / model / trigger）是 schedule 而非 connection params，建议抽 `<WindowAlignmentSection>` 单列，不要混进 connection form（语义不同：一个是凭据材料，一个是触发调度）。
- **不要泛化成任意 provider 配置框架**：只对"确有连接参数的 provider"建 form；纯 OAuth/keychain 的 provider（claude/codex）不需要 connection form。

## Before / After

```text
BEFORE  ProviderPage config modal
  if configId==="copilot"     → copilotToken state + effect + save
  if configId==="opencode-go" → workspaceId/authCookie state + effect + save
  if isApiKeyProvider         → providerApiKey state + effect + save
   ↑ 分支墙，一个 provider 的连接参数知识被横切

AFTER
  modal: providerId → <ConnectionForm />   ← 注册表（呼应后端 adapters 注册表）
     ├─ <CopilotTokenForm />         自管输入/保存态
     ├─ <OpenCodeGoConnectionForm />
     └─ <ApiKeyForm />
  useProviderDisplayPrefs()  ← order/cardVisible/trayVisible/colCount/trayMetric 收敛
```

## Acceptance criteria

- [ ] config modal 不再用 `configId === "..."` 分支渲染各 Provider 的连接参数表单；改为 `providerId → Form` 映射。
- [ ] 每个 Provider 的连接参数编辑是独立深组件，自管输入态与保存态。
- [ ] 新增一个带连接参数的 Provider 时，只需新增一个 form 文件 + 注册表一行，不改 modal 主体。
- [ ] connection form 可注入 fake `onSave` 单测，不依赖 Tauri runtime。
- [ ] display-preferences 收敛为 `useProviderDisplayPrefs`，ProviderPage 顶层不再裸露多组偏好 record state。
- [ ] 对外行为不变：Copilot token / OpenCode Go workspace+cookie / API key 保存、卡片排序/隐藏、tray 偏好均与现状一致。

## Out of scope

- 不改任何 Provider 的 quota 拉取或凭据后端语义。
- 不改窗口对齐（window alignment）的调度逻辑，仅将其 UI 从 connection 混杂中分离。
- 不动 ProjectPage / SyncPage。
- display-preferences 的后端归属调整仍由 `[[260624-1509-provider-display-preferences-return-to-provider-domain]]` 跟踪；本 issue 只收敛前端读写形态。

## Notes

ProviderPage 与后端 provider_quota 的 friction 同构：**Provider 是天然的垂直切分轴**，前后端都该按 Provider 收敛知识而非按"技术层"摊开。建议前端先做连接参数 form 拆分（用户可见路径最短、最易回归验证），display-prefs hook 可与后端 `[[260624-1509-provider-display-preferences-return-to-provider-domain]]` 同期推进以保持语义一致。
