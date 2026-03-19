# Skill: wtp-workspace-operator

Use this skill when an agent needs to create, inspect, reuse, navigate, or remove a `wtp` workspace while working across one or more repositories.

## Use this skill for

- starting a new multi-repo task workspace
- checking whether a workspace already exists
- inspecting what repositories are attached to a workspace
- choosing whether to reuse or create a workspace
- removing a workspace after the work is done

## Core model

A workspace is a directory under:

`<workspace_root>/<workspace_name>/`

Attached repositories live under:

`<workspace_root>/<workspace_name>/<repo_slug>/`

`wtp` stores metadata in:

`<workspace_root>/<workspace_name>/.wtp/worktree.toml`

## Primary commands

- `wtp ls`
- `wtp ls --short`
- `wtp create <name>`
- `wtp status`
- `wtp rm <name>`

## Operating rules

1. Start by discovering state.
   - Use `wtp ls --short` when the goal is simple existence/completion logic.
   - Use `wtp ls` or `wtp status` when the user needs a readable summary.

2. Prefer reuse over unnecessary creation.
   - If a workspace with the intended task name already exists and matches the task, reuse it.
   - Avoid creating duplicate workspaces with slightly different names unless the user asked for isolation.

3. Do not depend on `wtp cd` for agent execution.
   - Instead, capture the printed workspace path from `wtp create` or inspect the configured workspace root.
   - After creation, continue by using that path as the next working directory.

4. Treat workspace removal as a cleanup operation, not a convenience shortcut.
   - Before `wtp rm`, inspect status if there is any chance the workspace contains unfinished changes.
   - Do not add `--force` unless the user explicitly authorized destructive cleanup.

## Recommended flow

### Create or reuse a workspace

1. Run `wtp ls --short`
2. If the target workspace exists, reuse it
3. Otherwise run `wtp create <workspace>`
4. Note the resulting workspace path for future commands

### Inspect a workspace before doing work

1. If already inside the workspace, run `wtp status`
2. Otherwise move into the workspace path first, then run `wtp status`
3. Report which repositories are attached and whether they appear dirty or clean

### Remove a workspace safely

1. Check whether the workspace contains active worktrees
2. Inspect status when there may be uncommitted changes
3. Run `wtp rm <workspace>`
4. Use `--force` only with explicit user approval

## Agent output expectations

When using this skill, report back:

- workspace name
- workspace path
- whether it was reused or newly created
- which repos are currently attached, if any
- whether cleanup required force or was blocked by dirty worktrees

## Anti-patterns

Avoid these mistakes:

- using `wtp cd` as the only navigation mechanism
- creating a new workspace without checking for an existing one
- removing a workspace without checking for dirty state when the risk is non-trivial
- assuming the current directory is already a workspace without verification
