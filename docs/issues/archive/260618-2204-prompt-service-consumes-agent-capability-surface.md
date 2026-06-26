# Prompt service consumes Agent Capability Surface

## What to build

Add the first backend Prompt capability slice so global Prompt discovery and Agent Matrix facts are derived from `Agent Capability Surface`, not from a new local copy of agent order or prompt file paths. The slice should let the app list global prompt assets through a backend service while preserving the current Prompt domain rule: MVP Prompt is global-only and uses one canonical source with target placements computed per agent.

## Acceptance criteria

- [x] A backend Prompt service lists global prompt assets from the prompt-capable agents declared by `Agent Capability Surface`.
- [x] Prompt discovery excludes generated symlink/copy placements and records only canonical source files.
- [x] Prompt Agent Matrix rows include the complete canonical agent set in `Agent Capability Surface` order.
- [x] Prompt target paths are computed from `Agent Capability Surface` prompt facts; callers cannot submit arbitrary target paths.
- [x] At least one behavior test proves a Copilot prompt source can produce target/none cells for other prompt-capable agents without duplicating agent order in the Prompt module.
- [x] Existing Skill behavior remains unchanged.

## Blocked by

None - can start immediately.

## Notes

Keep this as a narrow Prompt backend slice. Do not introduce cross-language code generation here. Do not expand Prompt into project-level prompts; `CONTEXT.md` still defines Prompt as global-only for MVP.

## Resolution

2026-06-26: Current code already supports project prompts, so the fix preserved that newer behavior instead of reverting to the older global-only note above. The remaining drift was the local Prompt SQL copy of agent order; `list_prompts` now sorts through the shared Agent Capability Surface order helper, and behavior tests cover Copilot source target/none cells plus source ordering by capability surface rather than prompt file name.
