# Provider display consumes Agent Capability Surface

## What to build

Make agent-backed Provider display facts consume `Agent Capability Surface` instead of carrying local provider/agent copies. The goal is not to implement full quota polling; it is to ensure Provider identity, credential hints, and "also an Agent" display facts come from the shared capability surface where they overlap with Agent definitions.

## Acceptance criteria

- [ ] Provider seed/list behavior derives agent-backed provider rows from agents with a Provider surface.
- [ ] Credential hints for CodeX, Copilot, OpenCode, and Claude Code come from `Agent Capability Surface`.
- [ ] Generic Agent does not appear as a Provider because it has no Provider surface.
- [ ] Existing non-agent providers can still be represented without pretending they are Agents.
- [ ] At least one behavior test proves Provider listing marks agent-backed providers consistently with `Agent Capability Surface`.
- [ ] The Provider page behavior remains compatible with current mock/demo data until a real Provider API fully replaces it.

## Blocked by

None - can start immediately.

## Notes

Keep quota polling and provider-specific connection params out of this ticket unless the minimum list behavior already requires them. `Agent Capability Surface` should own shared agent/provider facts, not third-party account lifecycle.
