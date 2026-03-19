# Cursor Integration

This guide shows how to adapt the `wtp` agent skills for Cursor.

## Goal

Give Cursor a stable rule set for when and how to use `wtp` while working across multiple repositories.

## Recommended approach

Cursor works best with short rules. Keep the full workflow in `skills/`, and add a compact Cursor rule that points to those files.

## Example Cursor rule

Create a rule file such as:

```text
.cursor/rules/wtp.mdc
```

Suggested contents:

```text
Use `wtp` for temporary multi-repo workspace orchestration.

Before acting, read the most relevant file under `skills/`:
- `skills/wtp-workspace-operator/SKILL.md`
- `skills/wtp-repo-attach/SKILL.md`
- `skills/wtp-safe-cleanup/SKILL.md`

Rules:
- Prefer direct workspace paths over `wtp cd`
- Use `wtp switch` for the current repo
- Use `wtp import` from inside a workspace for another repo
- Use `wtp status` before cleanup
- Do not use `--force` without explicit user approval
```

## Why this split works well in Cursor

- Cursor rules stay short and always-on
- detailed `wtp` behavior stays in local skill files
- the same skill content remains reusable across Codex and Claude Code

## Prompt examples

- "Use `wtp` to create or reuse a workspace for this task."
- "Attach the current repo to the workspace using the local `wtp` guidance."
- "Safely remove this workspace and tell me if force would be required."
