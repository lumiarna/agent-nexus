# Add Pi agent

## Goal

Add `Pi` as a first-class Agent in Agent Nexus so it appears in the Agent Matrix and related UI surfaces alongside the existing canonical agents.

## Confirmed Facts

- Agent capabilities are defined in `crates/nexus-core/src/services/agent_capabilities.rs` and exposed to the frontend through the Tauri command `src-tauri/src/commands/agent_capabilities.rs`.
- Frontend canonical agent definitions live in `src-react/src/config/agents.ts`.
- Agent brand marks used by the Agent Matrix live in `src-react/src/components/ui/agent-logo.tsx`; Qoder uses an external SVG file in `src-react/public/`.
- `Pi` is already referenced by Trellis workflow docs as a supported platform, but it is not yet present in the product agent definitions.
- Pi’s documented config layout uses `~/.pi/agent` as global config dir, `~/.pi/agent/skills` as global skills dir, `.pi/skills` as project skills dir, and `~/.pi/agent/AGENTS.md` as global prompt file.
- Pi loads project context from `AGENTS.md`, but project prompt UI currently collapses `AGENTS.md` ownership to Generic Agent; Pi should not add a separate project-prompt matrix column if that would duplicate Generic Agent.
- Existing help copy mentions only the current canonical agents in at least `src-react/src/components/skill/SkillPage.tsx`.
- Existing extra prompt file help text in `src-react/src/components/project/ProjectDetailView.tsx` explicitly documents only `AGENTS*.md` (Generic Agent) and `CLAUDE*.md` (Claude Code); this reflects current source-agent ownership rules for extra prompt files.
- The Pi SVG reference provided by the user is `C:\Users\SONGSH2\AppData\Local\Zed\external_agents\registry\icons\pi-acp.svg`.

## Requirements

1. Add `Pi` to the backend Agent Capability Surface with canonical display metadata and filesystem surfaces.
2. Add `Pi` to the frontend canonical agent config so TypeScript agent unions, display order, and Skill/Prompt Agent Matrix rendering include it.
3. Add a Pi brand mark for the frontend Agent UI, reusing the user-provided SVG as the source reference.
4. Update any user-facing copy that enumerates canonical agents so `Pi` is included where appropriate.
5. Keep Pi modeled as an `Agent`, not a `Provider`.
6. Do not change extra prompt file source-agent semantics unless implementation evidence shows that Pi requires a broader behavior change.
7. Do not add a separate Pi column to the project-scope Prompt matrix when it would duplicate Generic Agent’s `AGENTS.md` namespace.
8. Generic Agent must not be user-disableable.

## Out of Scope

- Adding a Pi provider/quota integration.
- Changing database schema or migration logic unless implementation reveals a hard requirement.
- Redesigning extra prompt file ownership rules across multiple AGENTS.md-based agents.

## Acceptance Criteria

- [ ] `list_agent_capabilities` returns a `Pi` entry with the expected display name and filesystem surfaces.
- [ ] Frontend agent config includes `Pi`, and `AgentName`/display order now include it.
- [ ] Pi appears in the Skill matrix and in prompt matrices where canonical prompt-capable agents are shown, without introducing a duplicate Pi column for the project-scope `AGENTS.md` namespace.
- [ ] Pi renders with a dedicated logo in the UI.
- [ ] Any visible copy that lists canonical agents is updated to include `Pi` where that list is intended to stay exhaustive.
- [ ] No existing Agent or Provider behavior regresses as part of the Pi addition.

## Decisions

- Canonical order: `Generic Agent / Claude Code / CodeX / Copilot / OpenCode / Pi / Qoder`.
- Pi should not get its own duplicate project-prompt matrix column when that surface is the same `AGENTS.md` namespace already represented by Generic Agent.
- Generic Agent should always remain enabled; disabling it is not allowed.
