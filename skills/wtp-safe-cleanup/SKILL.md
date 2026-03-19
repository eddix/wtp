# Skill: wtp-safe-cleanup

Use this skill when an agent needs to clean up a `wtp` workspace or detach repositories from it without surprising the user.

## Use this skill for

- removing a single attached repo from a workspace
- removing an entire workspace
- deciding whether `--force` is appropriate
- reporting blocked cleanup because of dirty worktrees

## Primary commands

- `wtp status`
- `wtp eject <repo>`
- `wtp rm <workspace>`

## Safety rules

1. Inspect before deleting when the workspace may contain active work.
   - Prefer `wtp status` before cleanup.

2. Treat dirty worktrees as a stop signal.
   - If `wtp eject` or `wtp rm` reports uncommitted changes, stop and report clearly.
   - Do not add `--force` unless the user explicitly asked for forced cleanup.

3. Use the narrowest cleanup that matches the request.
   - If the user wants to remove one repo from a workspace, use `wtp eject`.
   - If the user wants to remove the whole workspace, use `wtp rm`.

4. Expect cleanup to involve underlying git worktree removal.
   - Failures may come from git state, missing directories, or worktree dirtiness.
   - Report these concretely instead of hiding them.

## Recommended flow

### Eject one repo

1. Run `wtp status` if current state is unclear
2. Identify the target repo by slug or display name
3. Run `wtp eject <repo>`
4. If blocked by dirty changes, stop and summarize what needs user approval or manual handling

### Remove a whole workspace

1. Inspect status or at least note attached repos
2. Run `wtp rm <workspace>`
3. If blocked by dirty worktrees, stop and report that `--force` would be destructive
4. Use `wtp rm <workspace> --force` only with explicit user permission

## Agent output expectations

Report back:

- cleanup scope: one repo or whole workspace
- target workspace
- whether cleanup succeeded, was blocked, or required force
- any repo/worktree that still needs manual attention

## Anti-patterns

Avoid these mistakes:

- defaulting to `--force`
- deleting the whole workspace when the user only asked to remove one repo
- reporting generic failure without mentioning dirty worktrees or git worktree issues
- assuming missing directories mean cleanup is risk-free
