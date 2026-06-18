# Agent Capability Surface cross-language SSOT decision

## What to build

Decide whether Rust and TypeScript Agent Capability Surface definitions should remain parallel module-level definitions or move to a generated/shared source. This is a decision ticket, not an implementation ticket; it should produce a documented choice with enough constraints for a later AFK implementation issue.

## Acceptance criteria

- [ ] Compare at least three options: keep parallel modules with tests, generate Rust/TypeScript from a shared data file, or expose the backend capability surface through IPC for frontend consumption.
- [ ] Document the expected source of truth for canonical order, config roots, Skill dirs, Prompt files, colors/abbrs, and Provider credential hints.
- [ ] Identify build complexity, type-safety, packaging, and offline frontend preview tradeoffs for each option.
- [ ] Record the decision in `docs/adr/` or the project’s chosen decision log.
- [ ] If generation or IPC is selected, create a follow-up AFK implementation issue with concrete acceptance criteria.

## Blocked by

- `docs/issues/260618-2204-prompt-service-consumes-agent-capability-surface.md`
- `docs/issues/260618-2205-provider-display-consumes-agent-capability-surface.md`

## Notes

Do not start by adding code generation. First let Prompt and Provider consume the backend capability surface so the real fact shape is stable enough to evaluate.
