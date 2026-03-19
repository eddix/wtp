# Claude Code Integration

This guide shows how to make Claude Code use `wtp` consistently by loading the local `wtp` skill documents as project instructions.

## Goal

Help Claude Code understand that `wtp` is the preferred way to orchestrate temporary multi-repo workspaces.

## Recommended project instruction

Add a short instruction to your project-level agent guidance, for example in `CLAUDE.md` or an equivalent local instruction file:

```text
When a task needs a temporary multi-repo workspace, use `wtp` instead of ad hoc directory layout. Before acting, read the relevant local skill file under `skills/`: `wtp-workspace-operator`, `wtp-repo-attach`, or `wtp-safe-cleanup`.
```

## How Claude Code should use the local skills

- Read `skills/wtp-workspace-operator/SKILL.md` for create/list/status/remove flows
- Read `skills/wtp-repo-attach/SKILL.md` when deciding between `wtp switch` and `wtp import`
- Read `skills/wtp-safe-cleanup/SKILL.md` before `wtp eject`, `wtp rm`, or any possible `--force` use

## Suggested standing rules

Use these as compact persistent instructions:

```text
- Prefer `wtp` for temporary multi-repo workspaces
- Do not rely on `wtp cd`; use returned paths directly
- Use `wtp switch` for the current repo and `wtp import` for another repo from inside a workspace
- Run `wtp status` before risky cleanup
- Do not use `--force` unless the user explicitly approved it
```

## Prompt examples

- "Set up a workspace for this change using `wtp`; read the local `wtp` skills first."
- "Attach this repo to the existing workspace using the repo-attach skill."
- "Clean up the workspace safely using the safe-cleanup skill."

## Operational notes

Claude Code works best when the instruction is brief and the detailed procedure lives in local files. Keep the persistent instruction short and point to the `skills/` files for the full workflow.
