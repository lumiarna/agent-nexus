# Local Issue Tracker

This repo tracks issues as markdown files under `.scratch/<feature>/`.

## Layout

- One directory per feature or workstream: `.scratch/<feature>/`
- One markdown file per issue inside that directory
- Start new issues from `.scratch/_templates/issue.md`

## Naming

- Directory names should be short, stable, and lowercase kebab-case.
- Issue filenames should be lowercase kebab-case summaries, for example: `project-key-derivation.md`

## Required metadata

Each issue must include frontmatter with at least these fields:

- `title`: human-readable issue title
- `status`: one of `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`
- `owner`: current owner, or `unassigned`
- `created`: ISO timestamp
- `updated`: ISO timestamp

## Workflow

- Create the feature directory if it does not exist.
- Copy `.scratch/_templates/issue.md` into the target feature directory.
- Fill in the frontmatter and sections.
- Update `status` and `updated` whenever the issue changes state.

## Notes

- This is the source of truth for local issue tracking.
- If the repo later moves to GitHub, GitLab, or another tracker, keep this directory only as migration history unless the workflow is explicitly retained.
