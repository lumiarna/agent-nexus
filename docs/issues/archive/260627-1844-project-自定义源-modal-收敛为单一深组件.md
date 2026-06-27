# Project 三个自定义源 modal 收敛为单一深组件

> 架构深化 issue（improve-codebase-architecture，前端）。推荐强度：**Strong**。
> 词汇遵循 `CONTEXT.md`（Project Custom Source / Extra Prompt File / Session Directory）与 codebase-design（module / interface / depth / locality）。

## 问题

`src-react/src/components/project/ProjectPage.tsx`（1752 行）内联实现了 ADR-0003 约定的三个自定义源 modal，每个都是手写一份：

| 源 | 状态（行号） | 增/删处理函数（行号） |
| --- | --- | --- |
| Custom skills dirs | `customDirsOpen` / `customDirInput` `261-262` | `addCustomSkillsDir` / `removeCustomSkillsDir` `432-463` |
| Extra prompt files | `extraFilesOpen` / `extraFileInput` `263-264` | `addExtraPromptFile` / `removeExtraPromptFile` `465-500` |
| Sessions dir（单值） | `sessionDirOpen` / `sessionDirInput` `265-266` | `submitSessionsDir` `502-515` |

前两个（skills dirs / prompt files）是**逐字同构的 string-list 编辑器**：

```text
add:    trim → [校验] → 去重(existing.includes) → mutateAsync([...existing, x]) → 清空输入 → toast
remove: mutateAsync(existing.filter(≠x)) → toast
```

唯一差异只有两处：extra-prompt 多一步 `matchesPromptGlob` 校验、mutation 参数 key 是 `dirs` vs `files`。其余结构、错误处理、toast 模式完全一致，是复制粘贴出来的。

> ADR-0003 已决策「**三个独立 modal**，每个 card 头部一个 configure 按钮，**与 Skill 已有 modal 模板一致**」——这条 UX 决策**不重新讨论**。本 issue 不动「三个 modal」的结论，只指出 ADR 没说它们要**各写一份实现**；现状恰恰背离了 ADR 自己说的"与模板一致"。

### deletion test

把这套「带输入框 + 列表 + 删除按钮 + 校验/去重」的 string-list 编辑逻辑抽成一个组件后删掉它 → add/remove/去重/toast 会在 skills-dirs 与 prompt-files 两处重新出现 ⇒ 复杂度集中，是真组件，不是 pass-through。

## What to build

一个 **深 presentational 组件**（小 props，藏住编辑行为），三个 modal 都是它的实例：

```tsx
type StringListConfigModalProps = {
  open: boolean;
  onClose: () => void;
  title: string;
  items: string[];
  onAdd: (value: string) => Promise<void>;     // 父层注入 mutateAsync
  onRemove: (value: string) => Promise<void>;
  validate?: (value: string) => string | null;  // 返回错误文案；prompt-files 传 matchesPromptGlob
  placeholder?: string;
  maxItems?: number;                             // sessions dir 用 1，退化为单值编辑
};
```

- skills-dirs：`items={dp.customSkillsDirs}`，无 `validate`。
- prompt-files：`items={dp.extraPromptFiles}`，`validate={matchesPromptGlob}`。
- sessions-dir：`maxItems={1}` 的单值变体（或保留一个 `SingleValueConfigModal` 兄弟组件，二选一在 grilling 决定）。

去重（`existing.includes`）、trim、清空输入、错误 toast 这些约定收敛进组件内部 —— 它们今天分散在每个 add 函数里，正是后端 `[[260627-1145-project-custom-skills-dir-global-propagation]]` 提到的「custom dirs 应去重、规范化」校验在前端的落点。**一处实现，技能/提示/未来新增的源都受益**（leverage）。

## Suggested shape

- **组件接受依赖、不自建**：`onAdd`/`onRemove` 由 `ProjectPage` 用既有的 `useSetProjectCustomSkillsDirsMutation` / `useSetProjectExtraPromptFilesMutation` / `useSetProjectSessionsDirMutation` 注入。组件本身无 react-query 依赖 → 可脱离后端单测（给 fake onAdd 断言去重/校验/清空行为）。
- **放在 `components/project/` 下**（如 `StringListConfigModal.tsx`），复用既有 `@/components/ui/modal` 的 `Modal`/`ModalHeader`/`ModalFooter` 原语。
- **不要泛化过头**：只服务「string 列表 / 单值」这一形态，不做成任意 schema 的通用表单框架（"one adapter = hypothetical seam"——目前只有这一种编辑形态）。

## Before / After

```text
BEFORE  ProjectPage.tsx
  customDirInput  + addCustomSkillsDir/removeCustomSkillsDir   ┐
  extraFileInput  + addExtraPromptFile/removeExtraPromptFile   ├─ 逐字同构，复制三份
  sessionDirInput + submitSessionsDir                          ┘
  去重/trim/toast 各写一遍

AFTER
  <StringListConfigModal />  ← 小 props，藏住 add/remove/去重/校验/toast
     ├─ skills dirs 实例
     ├─ prompt files 实例（validate=matchesPromptGlob）
     └─ sessions dir 实例（maxItems=1）
```

## Acceptance criteria

- [ ] 三个自定义源 modal 由同一个深组件（含单值变体）渲染，不再各写一份 add/remove。
- [ ] 去重、trim、清空输入、错误 toast 逻辑只存在于该组件内部一处。
- [ ] prompt-files 的 `matchesPromptGlob` 校验通过 `validate` prop 传入，不在组件内硬编码 prompt 概念。
- [ ] 组件可在不依赖 react-query / Tauri 的前提下单测（注入 fake `onAdd`/`onRemove`）。
- [ ] ProjectPage 中与三个源相关的 `*Open`/`*Input` 局部 state 数量下降（由组件自管输入态）。
- [ ] 对外行为不变：三个 configure 入口、modal 文案、增删结果与现状一致（ADR-0003 的"三个独立 modal"UX 不变）。

## Out of scope

- 不改 ADR-0003 的「三个独立 modal」决策，不合并成单一 modal。
- 不改后端 `set_project_*` IPC 或校验语义。
- 不动 ProjectPage 其余 feature（见 `[[260627-1844-projectpage-god-component-按-feature-抽-hooks]]`）。

## Notes

这是 ProjectPage 拆解里**收益最直接、风险最低**的一刀：纯 presentational、可单测、不碰数据层。建议作为前端深化的第一步，做完后 `[[260627-1844-projectpage-god-component-按-feature-抽-hooks]]` 里 ProjectPage 会少 ~6 个 useState 与 ~80 行内联逻辑。
