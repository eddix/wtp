# Codex Integration

This guide shows how to make the `wtp` agent-facing skills in this repository usable from Codex-style workflows.

## Goal

Teach Codex to treat `wtp` as a workspace orchestration tool rather than a generic CLI.

The three core skills are:

- `skills/wtp-workspace-operator/SKILL.md`
- `skills/wtp-repo-attach/SKILL.md`
- `skills/wtp-safe-cleanup/SKILL.md`

## Recommended setup

### Option 1: Keep the skills in this repository

Use this when the agent is already working in the `wtp` repository and can read local files directly.

In that case, instruct the agent to read the relevant `SKILL.md` file before using `wtp`.

Example prompts:

- "Before using `wtp`, read `skills/wtp-workspace-operator/SKILL.md` and follow it."
- "Use `skills/wtp-repo-attach/SKILL.md` before attaching repos with `wtp`."

### Option 2: Install these as Codex skills

If you want the skills available across repositories, copy each skill directory into your Codex skills home.

A typical destination is:

```bash
~/.codex/skills/
```

Suggested installed layout:

```text
~/.codex/skills/
├── wtp-workspace-operator/
│   └── SKILL.md
├── wtp-repo-attach/
│   └── SKILL.md
└── wtp-safe-cleanup/
    └── SKILL.md
```

## Suggested trigger descriptions

If you convert these into standalone Codex skills, use descriptions that trigger on real agent tasks.

### `wtp-workspace-operator`

Use when the user wants to create, inspect, reuse, navigate, or remove a `wtp` workspace for multi-repo work.

### `wtp-repo-attach`

Use when the user wants to attach the current repository or another repository to a `wtp` workspace and the agent must decide between `wtp switch` and `wtp import`.

### `wtp-safe-cleanup`

Use when the user wants to clean up a `wtp` workspace or eject a repository from it, especially when dirty worktrees or `--force` may be involved.

## Prompting pattern

A good system or project instruction is short and behavior-oriented.

Example:

```text
When a task involves creating, attaching, inspecting, or cleaning up `wtp` workspaces, use the installed `wtp` skills first. Prefer direct workspace paths over `wtp cd`, distinguish `wtp switch` from `wtp import`, and do not use `--force` without explicit user approval.
```

## Operational notes

- Prefer direct paths and working-directory changes over `wtp cd`
- Use `wtp ls --short` and `wtp status` as lightweight discovery commands
- Use `wtp switch` for the current repo
- Use `wtp import` from inside a workspace when attaching another repo
- Treat `wtp eject` and `wtp rm` as cleanup operations with safety checks
