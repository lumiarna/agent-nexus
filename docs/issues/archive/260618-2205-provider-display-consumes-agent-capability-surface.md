# Provider display consumes Agent Capability Surface

## What to build

Make agent-backed Provider display facts consume `Agent Capability Surface` instead of carrying local provider/agent copies. The goal is not to implement full quota polling; it is to ensure Provider identity, credential hints, and "also an Agent" display facts come from the shared capability surface where they overlap with Agent definitions.

## Acceptance criteria

- [x] Provider seed/list behavior derives agent-backed provider rows from agents with a Provider surface.
- [x] Credential hints for CodeX, Copilot, and Claude Code come from `Agent Capability Surface`.
- [x] Generic Agent does not appear as a Provider because it has no Provider surface.
- [x] Existing non-agent providers can still be represented without pretending they are Agents.
- [x] At least one behavior test proves Provider listing marks agent-backed providers consistently with `Agent Capability Surface`.
- [x] The Provider page behavior remains compatible with current mock/demo data until a real Provider API fully replaces it.

## Blocked by

None - can start immediately.

## Notes

Keep quota polling and provider-specific connection params out of this ticket unless the minimum list behavior already requires them. `Agent Capability Surface` should own shared agent/provider facts, not third-party account lifecycle.

## Resolution

2026-06-26: Current code treats `OpenCode` as an Agent without Provider surface and `OpenCode Go` as a non-agent Provider, so the outdated OpenCode credential-hint criterion was corrected above. Provider display rows now have behavior coverage proving agent-backed identity and credential hints come from Agent Capability Surface while non-agent providers such as OpenCode Go remain representable.
