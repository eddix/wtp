# Skill: wtp-repo-attach

Use this skill when an agent needs to attach a repository to a `wtp` workspace.

This skill exists because `wtp switch` and `wtp import` solve different problems and should not be mixed up.

## Use this skill for

- attaching the current repository to a workspace
- attaching another repository while already inside a workspace
- deciding whether to use `switch` or `import`
- handling host aliases and default host behavior

## Command split

### Use `wtp switch` when

- the agent is currently inside the repository it wants to attach
- the source repository is the repo under the current working directory
- the agent wants `wtp` to create a worktree for the current repo in the target workspace

Example:

```bash
cd ~/code/my-repo
wtp switch feature-x
```

### Use `wtp import` when

- the agent is already operating inside a workspace
- the agent wants to attach another repository into that existing workspace
- the target repo is referenced by host alias, default host, or explicit path

Example:

```bash
cd ~/.wtp/workspaces/feature-x
wtp import company/another-repo
```

## Primary commands

- `wtp switch <workspace>`
- `wtp switch --create <workspace>`
- `wtp import <path>`
- `wtp import --repo <absolute-or-relative-path>`
- `wtp status`

## Decision rules

1. If the current directory is inside the repository to attach, prefer `wtp switch`.
2. If the current directory is already a workspace and the repo to attach is elsewhere, prefer `wtp import`.
3. If the workspace may not exist yet and the current repo should be the first attached repo, prefer `wtp switch --create <workspace>`.
4. If using repo shorthand like `company/project`, make sure host alias behavior is understood:
   - `--host` overrides everything
   - otherwise `default_host` may be used
   - if no host is configured, a plain path may be treated as a filesystem path instead

## Safety and correctness checks

Before attaching a repo:

- verify whether you are in a git repository if planning to use `wtp switch`
- verify whether you are in a workspace directory if planning to use `wtp import` (it always auto-detects the workspace from the current directory — there is no flag to override this)
- inspect existing workspace contents if there is risk of duplicate attachment
- remember that one repository can only have one worktree per workspace

## Branch behavior

- If no branch is specified, `wtp` generally defaults to the workspace name
- If no base is specified, `wtp` resolves one from the current branch or HEAD context
- If a branch already exists, `wtp` may reuse it instead of creating a new one

The agent should avoid assuming details that `wtp` can determine itself unless the user explicitly requested a branch or base.

## Recommended flow

### Attach current repo to workspace

1. Confirm the current directory is in the intended repo
2. Run `wtp switch <workspace>` or `wtp switch --create <workspace>`
3. Capture the created worktree path from command output
4. Continue subsequent work in that new worktree path

### Attach another repo to an existing workspace

1. Move into the workspace root directory
2. Run `wtp import <repo-ref>` or `wtp import --repo <path>`
3. Run `wtp status` if you need a summary of the updated workspace
4. Continue in the specific attached repo directory under the workspace

## Agent output expectations

Report back:

- whether `switch` or `import` was used
- target workspace name and path
- attached repo identity
- resulting worktree path
- any branch/base information that materially affected the result

## Anti-patterns

Avoid these mistakes:

- using `wtp import` from a normal repo directory when the intent was to attach the current repo
- using `wtp switch` while not inside the source repository
- assuming host aliases exist without checking configuration
- forcing branch/base parameters when `wtp` defaults are sufficient
