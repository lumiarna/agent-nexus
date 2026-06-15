# Issue Tracker

This repo uses a local-markdown issue tracker.

## Source of truth

Issues live as markdown files under `.scratch/<feature>/` in this repo.

## How agents should work with issues

- Read and write issue content directly in the repo instead of calling a remote issue API.
- Treat `.scratch/` as the issue-tracker workspace.
- Prefer creating one markdown file per issue.
- Preserve any existing local conventions if `.scratch/` already contains issue files later.

## Workflow notes

- Use local markdown for issue creation, refinement, triage, and status updates.
- If this repo later adopts GitHub, GitLab, or another tracker, update this file and the `## Agent skills` block in `CLAUDE.md`.
