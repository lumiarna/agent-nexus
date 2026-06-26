# Agent Capability Surface cross-language SSOT decision

## What to build

Decide whether Rust and TypeScript Agent Capability Surface definitions should remain parallel module-level definitions or move to a generated/shared source. This is a decision ticket, not an implementation ticket; it should produce a documented choice with enough constraints for a later AFK implementation issue.

## Acceptance criteria

- [x] Compare at least three options: keep parallel modules with tests, generate Rust/TypeScript from a shared data file, or expose the backend capability surface through IPC for frontend consumption.
- [x] Document the expected source of truth for canonical order, config roots, Skill dirs, Prompt files, colors/abbrs, and Provider credential hints.
- [x] Identify build complexity, type-safety, packaging, and offline frontend preview tradeoffs for each option.
- [x] Record the decision in `docs/adr/` or the project’s chosen decision log.
- [x] If generation or IPC is selected, create a follow-up AFK implementation issue with concrete acceptance criteria.

## Blocked by

- `docs/issues/260618-2204-prompt-service-consumes-agent-capability-surface.md`
- `docs/issues/260618-2205-provider-display-consumes-agent-capability-surface.md`

## Notes

Do not start by adding code generation. First let Prompt and Provider consume the backend capability surface so the real fact shape is stable enough to evaluate.

## Resolution

2026-06-26: Decision recorded in `docs/adr/0003-agent-capability-cross-language-ssot.md`. Runtime SSOT is the Rust backend Agent Capability Surface exposed through IPC; TypeScript definitions remain preview fallback/type support. Follow-up AFK issue created at `docs/issues/260626-1913-agent-capability-frontend-ipc-ssot-hardening.md`.
