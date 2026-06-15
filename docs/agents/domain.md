# Domain Docs

This repo uses a single-context domain-doc layout.

## Layout

- Root context file: `CONTEXT.md`
- Root ADR directory: `docs/adr/`

## Consumer rules

- Skills should read `CONTEXT.md` at the repo root to learn the project's domain language.
- Skills should read `docs/adr/` when it exists to understand architectural decisions.
- If `docs/adr/` does not exist yet, treat the repo as having no recorded ADR history rather than searching for alternate ADR locations.
- Do not assume a multi-context layout unless this repo later adds a root `CONTEXT-MAP.md`.
