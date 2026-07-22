# Frontend Type Safety

## 适用范围

适用于 `src-react/src/types/`、`src-react/src/config/`、`src-react/src/lib/api/` 和组件 props 类型。

## 类型来源

- 共享领域类型集中在 `types/index.ts`，字段命名与 Tauri IPC payload / `CONTEXT.md` glossary 对齐。
- Agent 名称来自 `config/agents.ts` 导出的 capability 类型；`types/index.ts` 用 `AgentName` 约束 `Cells`。
- API 层每个函数声明返回类型，例如 `lib/api/sync.ts` 返回 `TaskGroup[]`、`Task` 等，组件不处理 `unknown` payload。

## 命名与领域术语

- UI 展示和类型注释应使用 canonical domain names：`Agent`、`Provider`、`Project`、`Skill`、`Prompt`、`Session`、`Distribution`、`Cloud`。
- `AgentName` 展示值必须使用 `CONTEXT.md` 确认的完整 canonical 名称，如 `Generic Agent`、`Claude Code`、`CodeX`、`Copilot`、`OpenCode`、`Pi`、`Qoder`；短 ID 仅用于实现层 provider id 或配置 key。若代码中出现尚未写入 `CONTEXT.md` 的 capability entry，新增规范时应先澄清领域身份，不要直接扩充 canonical list。
- `LocationType` 使用 `Local` / `Cloud`；主内容 UI 不把 `WebDAV` 当作用户手选 location type。

## Runtime validation 现状

- 项目当前未引入 Zod / React Hook Form；表单校验以受控组件 + 纯规则 module 为主。
- 后端是最终校验层；前端类型不能替代 `nexus-core` 的 service validation。

## 常见错误 / anti-pattern

- 不要用 `any` 绕过 IPC payload 类型；若后端新增字段，先扩展 `types/index.ts` 或领域局部类型。
- 不要把实现层短 ID 当展示类型，例如把 `claude` 显示成 Agent 名称。
- 不要给 `Project` 身份绑定本地 path；`CONTEXT.md` 明确 `Project Key` 是稳定身份，`Project Path` 可变。

## Scenario: Canonical Agent capability expansion

### 1. Scope / Trigger
- Trigger: 新增或调整 canonical `Agent`，且该 Agent 会穿透 `nexus-core` capability surface、Tauri command、前端 `AGENTS` config、Agent Matrix 和 Settings 展示。

### 2. Signatures
- Backend capability surface：`crates/nexus-core/src/services/agent_capabilities.rs`
  - `AgentCapabilitySurface { name, abbr, color, config_dir, skill?, prompt?, provider? }`
  - `SkillSurface { global_dir, project_dir }`
  - `PromptSurface { global_file, project_file? }`
- Frontend canonical config：`src-react/src/config/agents.ts`
  - `AgentDef { name, abbr, color, dirs, surfaces, projectSkillDir?, projectPromptFile?, providerId?, authFile? }`
  - `AGENT_ORDER` 必须直接从 `AGENTS` 导出，避免另一份手写顺序。
- Matrix consumers：`src-react/src/components/ui/agent-icon.tsx` / `SkillPage.tsx` / `PromptPage.tsx`
- Settings preferences：`src-react/src/lib/api/agentPreferences.ts`
  - `AgentDisplayPreferences { disabled: string[]; defaultGlobalEntryAgent?: string }`

### 3. Contracts
- 新 canonical Agent 必须同时在后端 capability surface 与前端 `AGENTS` 中落地；只改其中一侧会导致 matrix、logo、types 或 settings 漂移。
- `Prompt` 的 project matrix 只展示真正拥有独立 project prompt namespace 的 Agent；共享 `AGENTS.md` namespace 的 Agent 不应额外新增重复列。
- `Generic Agent` 是 canonical-leftmost baseline；即使 `AgentDisplayPreferences.disabled` 允许其他 Agent 被隐藏，`Generic Agent` 也必须保持 enabled。
- `dirs` / `config_dir` / `skill` / `prompt` 使用真实消费端路径，不把 provider 凭据文件误写成 config root。

### 4. Validation & Error Matrix
- Backend 新增 Agent，但前端未更新 `AGENTS` / `AgentLogo` -> matrix/types/logo/settings 显示不完整。
- Frontend 新增 `projectPromptFile`，但该 Agent 只是复用 `AGENTS.md` namespace -> project prompt matrix 出现重复列，违背 product contract。
- Settings 允许 `Generic Agent` toggle off -> Agent Matrix 左侧基准列消失，默认 Global entry fallback 语义失真。
- `disabled` 中包含未知 canonical name -> 后端 validation 必须拒绝，前端不能假设字符串永远合法。

### 5. Good/Base/Bad Cases
- Good: 新增 `Pi` 时，同时更新 backend capability、frontend `AGENTS`、logo、matrix copy，并让它出现在 Skill + global Prompt matrix，但不出现在 project Prompt duplicate column。
- Base: 只新增一个纯 global prompt-capable Agent，`projectPromptFile` 留空/缺省，project prompt matrix 不变。
- Bad: 看到 Agent 实际也读取 `AGENTS.md`，就直接把 `projectPromptFile: "AGENTS.md"` 加进前端 config，结果 project prompt matrix 重复表示 Generic Agent namespace。

### 6. Tests Required
- Backend tests in `crates/nexus-core/tests/agent_capabilities.rs`
  - assert canonical order
  - assert new Agent surfaces (config dir, skill dirs, prompt files, provider presence/absence)
- Backend tests in `crates/nexus-core/tests/app_config.rs`
  - assert `Generic Agent` cannot be disabled
  - assert unknown agent names in preferences are rejected
- Frontend verification
  - typecheck must pass after `AgentName` union expands
  - review `PromptPage` project/global matrix agent sources separately so duplicate project prompt columns do not slip in

### 7. Wrong vs Correct
#### Wrong
```ts
agent({
  name: "Pi",
  projectPromptFile: "AGENTS.md",
  surfaces: ["skill", "prompt"],
  // ...
})
```
- 在当前产品里这会让 project Prompt matrix 把 `AGENTS.md` namespace 展示两次。

#### Correct
```ts
agent({
  name: "Pi",
  surfaces: ["skill", "prompt"],
  projectSkillDir: ".pi/skills",
  dirs: [
    { key: "PI_CONFIG_DIR", value: "~/.pi/agent" },
    { key: "PI_SKILLS_DIR", value: "~/.pi/agent/skills", derivedFrom: "PI_CONFIG_DIR" },
    { key: "PI_PROMPT_FILE", value: "~/.pi/agent/AGENTS.md", derivedFrom: "PI_CONFIG_DIR" },
  ],
})
```
- Pi 仍是 prompt-capable Agent，会出现在 global Prompt matrix；project Prompt matrix 继续由 Generic Agent 表示 `AGENTS.md` namespace。

## Scenario: Project custom Skill 判别联合与 typed intent

### 1. Scope / Trigger
- Trigger：修改 Skill row、Project custom canonical / incoming 渲染、传播 modal、Skill query/mutation 或相关 IPC payload。

### 2. Signatures
- `Skill` 是严格判别联合：
  - `agentCanonical { rowKey, skill, context, sourceAgent, cells: AgentCells }`
  - `projectCustomCanonical { rowKey, skill, sourceProject, destinations }`
  - `projectCustomIncoming { rowKey, skill, sourceProject, targetProject, cells: PlacementCells }`
- `skill.skillId` 在三 variant 中都是 canonical backend id；`rowKey` 只用于 React identity。
- `AgentCells` 可含 `source | target | none`；`PlacementCells` 只能 `target | none`。
- `ProjectCustomSkillDestination = { kind: "global" } | { kind: "project"; projectId: string }`。
- `ProjectCustomSkillIntent = setTargetEnabled | setAgentPlacement`；mutation 返回 `{ changed, skills: Skill[] }`。

### 3. Contracts
- 页面必须按 `kind` exhaustive render / select；不得恢复 optional `canonicalSkillId`、`placementScope`、`placementProjectId` 或 composite identity fallback。
- Project custom canonical 的 modal 直接消费 core eager `destinations`；页面不 join Skills、Projects、Settings，不读取默认入口 Agent，不循环撤销 cell。
- incoming row 直接使用 `skill.skillId + targetProject.id` 构造 `setAgentPlacement` intent；Open / Reveal / DMI 也统一使用 `skill.skillId`。
- 所有 Skill 数据 mutation 返回权威完整 catalog，并整体替换 `skillKeys.all`；Project custom intent 与显式 `scan_skills` 还 invalidate `projectKeys.all`。
- `useSkillsQuery` 必须调用只读 `list_skills`；会扫描文件系统并改写 sources/distributions 的 `scan_skills` 只能由显式 Refresh mutation 调用。
- record/batch record/delete/reorder/续认 Project Path 会改变 eager destination，成功后必须 invalidate `skillKeys.all`；纯 Git Base Folder 列表变化不需要。

### 4. Validation & Error Matrix
- Project custom cells 出现 `source` → TypeScript 类型错误；不得用 `as` / `any` 绕过。
- 缺少 `projectId` 的 Project destination → payload 无法满足判别联合；不得传空字符串占位。
- backend 返回 Validation / IO / Database / Reconciliation → mutation 保持失败，不能替换成功 cache 或伪造局部 row。
- scan 失败 → 保留旧 Skill cache，不 invalidate 成功态 Project 数据。
- 非 Tauri runtime 调用 intent → 由 typed API adapter 返回统一 runtime error，组件不直接调用 `invoke`。

### 5. Good/Base/Bad Cases
- Good：incoming row 点击 Agent cell，只提交一次 `setAgentPlacement`，成功后以响应中的完整 catalog 替换 cache。
- Base：普通列表读取只调用 `list_skills`，窗口聚焦 refetch 不扫描文件系统。
- Bad：`useSkillsQuery` 调用 `scan_skills`，导致普通 refetch 获取 mutation lock、改写数据库并触发跨领域副作用。

### 6. Tests Required
- TypeScript fixture 固定三 variant camelCase wire shape，并证明 Placement cells 不能表达 `source`。
- visibility / project selector 按 variant exhaustive。
- SkillRow 测试只断言一次 typed intent 委托、不含 `defaultAgent`，不复制 target join 或补偿规则。
- query 测试固定完整 catalog 替换、Project invalidation、list/scan 职责分离与失败不污染 cache。

### 7. Wrong vs Correct
#### Wrong
```ts
// 普通 query 不得执行会改写 source/distribution 的 scan。
useQuery({ queryKey: skillKeys.all, queryFn: () => skillsApi.scan() });
```

#### Correct
```ts
useQuery({ queryKey: skillKeys.all, queryFn: () => skillsApi.list() });
useMutation({
  mutationFn: () => skillsApi.scan(),
  onSuccess: (skills) => queryClient.setQueryData(skillKeys.all, skills),
});
```

## 场景：Settings Agent Config Root 打开目录

### 1. 范围 / 触发条件

- 触发条件：Settings 中的 Agent `CONFIG_ROOT` 需要在文件管理器中打开。

### 2. 签名

- Capability wire type：`AgentCapabilitySurface.name: AgentName`；Settings 不得把任意 `string` 强制断言成 canonical Agent。
- 前端 typed API：`agentCapabilitiesApi.openConfigRoot(name: AgentName): Promise<void>`。
- Tauri command：`open_agent_config_root(name: String) -> AppResult<()>`。
- Core helper：`resolve_agent_config_root(name: &str) -> AppResult<PathBuf>`，内部只使用共享 `services::paths::resolve_local_path`。

### 3. 契约

- 前端仅传递 `AgentName` canonical 名称，不能传递用户可控的路径。
- `services::paths::home_dir` / `resolve_local_path` 是本地路径的唯一共享解析链：Windows 优先原生 `USERPROFILE`、缺失时回退 `HOME`，非 Windows 只使用 `HOME`；Config Root 不得新增功能专用 resolver。
- Core 通过 `agent_by_name` 取 capability 的 `config_dir`，使用共享 resolver 展开并验证目录；Tauri shell 只以既有 `services::system_open::open_path` 打开，不新增第二套 opener。
- 非 Tauri runtime 由 `invokeCommand` 统一返回 desktop runtime 错误，Settings 页面 toast 该错误。

### 4. 校验与错误矩阵

- 未知 Agent 名称 -> `Validation("unknown agent: ...")`，不访问文件系统。
- Config Root 不存在 -> 明确的 `config root ... does not exist` 错误，不启动文件管理器。
- Config Root 不是目录 -> 明确的 `config root ... is not a directory` 错误。
- 系统 opener 的任意目标不存在或 OS 启动失败 -> 传播明确 `AppResult`，由页面 toast。
- 浏览器预览 -> `Agent Nexus desktop runtime is required for this action.`。

### 5. 正常 / 基础 / 错误案例

- 正常：点击 `Pi` 的 `CONFIG_ROOT`，API 传递 `"Pi"`，core 打开展开后的 `~/.pi/agent` 目录。
- 基础：`CONFIG_ROOT` 以外的 Agent 路径继续只是展示文本。
- 错误：前端将 `configDir` 或任意输入路径作为 command 参数，导致 desktop shell 成为任意路径打开接口。
- 错误：为 Explorer 单独实现 `HOME` 转换或新 opener，造成其他路径消费者继续使用不同语义。

### 6. 必需测试

- `services::paths` 单元测试：Windows 下 `HOME=/c/Users/...` 与 `USERPROFILE=C:\Users\...` 并存时，共享 `resolve_local_path` 必须使用 `USERPROFILE`。
- `crates/nexus-core/tests/agent_capabilities.rs`：创建真实目录后断言 canonical Agent 配置根；覆盖未知 Agent、目录缺失和目标不是目录。
- `services::system_open` 单元测试：缺失目标在启动系统 handler 前失败。
- `src-react/tests/component/settingsConfigRootOpen.test.tsx`：点击路径传递 canonical Agent 名；浏览器预览显示统一 runtime toast。

### 7. 错误与正确示例

#### 错误

```ts
invokeCommand("open_agent_config_root", { path: agent.configDir });
```

#### 正确

```ts
agentCapabilitiesApi.openConfigRoot("Generic Agent");
```

后端以 canonical Agent 名称查找 capability，经唯一共享路径解析器与唯一 system opener 打开配置根目录，避免调用方指定文件系统目标。

## 验证

- `src-react/package.json` 提供 `typecheck`：`tsc --noEmit`。
- 测试编译使用 `tsconfig.test.json`，纯 module 需保持 Node 可编译，避免依赖浏览器/Tauri runtime。
- 纯业务规则单测跑在 `node:test`（`pnpm test:unit`）；凡 import 链触及 `@/` 别名（如 `lib/tokens.ts`）的纯函数无法进 node harness（运行时无法解析 `@/`），需改为相对 import 或用 vitest component harness（`tests/component/**/*.test.tsx`）覆盖——见 `tsconfig.test.json` include 白名单与 `vitest.config.ts`。
