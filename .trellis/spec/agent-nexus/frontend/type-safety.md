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

## 验证

- `src-react/package.json` 提供 `typecheck`：`tsc --noEmit`。
- 测试编译使用 `tsconfig.test.json`，纯 module 需保持 Node 可编译，避免依赖浏览器/Tauri runtime。
