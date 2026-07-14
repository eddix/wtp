---
name: wtp-configure
description: Inspect or change wtp configuration - host aliases (wtp host), default host, workspace root, on_create hooks, config file precedence. Use when configuring wtp or resolving multiple-config-file warnings.
---

# Skill: wtp-configure

Use this skill when an agent needs to inspect or change `wtp` configuration: host aliases, the default host, the workspace root, workspace-creation hooks, or display options.

## Use this skill for

- inspecting the effective configuration and which config file is loaded
- adding, listing, or removing host aliases
- setting the default host for repo shorthand resolution
- configuring or skipping the `on_create` workspace hook
- resolving "multiple config files" warnings

## Configuration model

`wtp` loads the **first** config file found in this order:

1. `~/.wtp.toml`
2. `~/.wtp/config.toml`
3. `~/.config/wtp/config.toml`

If more than one exists, `wtp` prints a warning on every run and still uses only the first. When saving without an existing file, `wtp` writes to `~/.wtp/config.toml`.

Config fields:

- `workspace_root` — where workspaces live (default: `~/.wtp/workspaces`)
- `[hosts.<alias>]` with `root` — host alias to repository root directory
- `default_host` — alias used when repo shorthand is given without `--host`
- `[hooks]` `on_create` — script run after `wtp create`
- `[display]` `repo_colors` — `auto` (default) / `always` / `never` for repo name colors in `wtp ls --long`

## What has a command vs what needs editing the file

- `wtp config` — **read-only** overview: which config file is loaded, workspace root, workspaces, hosts, default host
- `wtp host add|ls|rm|set-default` — the only settings with write commands
- `workspace_root`, `[hooks]`, `[display]` — no subcommand exists; edit the loaded config file directly (check the `Config file` line in `wtp config` output first)

## Host aliases

A host alias maps a short name to a root directory so `wtp import` can take shorthand like `company/project` instead of an absolute path:

```bash
wtp host add gh ~/codes/github.com   # gh:owner/repo -> ~/codes/github.com/owner/repo
wtp host ls
wtp host set-default gh              # shorthand without --host now resolves via gh
wtp host rm gh
```

Rules enforced by `wtp host add`:

- alias must be simple — no spaces or slashes
- the path is tilde-expanded and made absolute; a non-existent path is accepted with a warning
- re-adding an existing alias fails; run `wtp host rm <alias>` first to replace it

Resolution order for repo shorthand: `--host` flag > `default_host` > interactive selection (errors in non-interactive terminals).

## Workspace creation hook

`[hooks] on_create` points at a script that runs after `wtp create` makes the directory:

- it must be executable, or the hook fails
- it receives `WTP_WORKSPACE_NAME` and `WTP_WORKSPACE_PATH` as environment variables
- a hook failure is reported as a warning — the workspace is still created
- `wtp create <name> --no-hook` skips it for one invocation

## Security fence

All destructive filesystem operations are confined to `workspace_root`. Operating on a workspace path outside that boundary triggers an interactive confirmation prompt — in a non-interactive terminal the prompt reads EOF and the operation is cancelled. Agents should treat "workspace outside workspace_root" as a hard stop and surface it to the user instead of retrying.

## Recommended flow

### Inspect configuration

1. Run `wtp config`
2. Note the `Config file` line — that is the only file worth editing
3. If a multiple-config-files warning appears, report the extra files and let the user decide which to keep

### Add a host alias

1. Run `wtp host ls` to check the alias is free
2. Run `wtp host add <alias> <root-path>`
3. If this is the primary host, run `wtp host set-default <alias>`
4. Confirm with `wtp host ls`

### Configure a create hook

1. Run `wtp config` to find the loaded config file
2. Add `[hooks]` with `on_create = "<script-path>"` to that file
3. Ensure the script is executable (`chmod +x`)
4. Verify with a throwaway `wtp create` / `wtp rm` cycle if the user wants proof

## Agent output expectations

Report back:

- which config file is in effect
- what was changed, via which mechanism (command vs file edit)
- current hosts and default host after the change
- any multiple-config-files warning that needs user attention

## Anti-patterns

Avoid these mistakes:

- editing a config file that is not the one `wtp config` reports as loaded
- re-running `wtp host add` on an existing alias instead of `rm` then `add`
- assuming a `wtp` subcommand exists to change `workspace_root` or hooks — those are file edits
- ignoring the multiple-config-files warning instead of surfacing it
- treating a fence confirmation prompt as a bug rather than a safety boundary
