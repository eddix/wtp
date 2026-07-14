---
name: wtp-setup
description: Install, upgrade, or verify the wtp binary and set up shell integration (wtp shell-init) and completions. Use when installing wtp, diagnosing a stale binary after an upgrade, or configuring a user's shell for wtp cd.
---

# Skill: wtp-setup

Use this skill when an agent needs to install, upgrade, or verify the `wtp` binary, or set up shell integration and completions for a human user.

## Use this skill for

- installing or upgrading `wtp` from this repository
- verifying which build of `wtp` is actually running
- setting up the shell wrapper that `wtp cd` requires
- generating shell completions for zsh, bash, or fish
- diagnosing "command not found" or stale-binary problems

## Primary commands

- `cargo install --path wtp-cli`
- `./install.sh`
- `wtp --version`
- `wtp shell-init`
- `wtp completions <zsh|bash|fish>`

## Install methods

There are two supported install paths, and they write to **different directories**:

| Method | Command | Installs to |
|---|---|---|
| cargo | `cargo install --path wtp-cli` | `~/.cargo/bin/wtp` |
| script | `./install.sh` | `~/.local/bin/wtp` |

The package in `wtp-cli/` is named `wtp` and produces the `wtp` binary. `install.sh` runs `cargo build --release -p wtp` and copies the result to `~/.local/bin/`.

If both locations are on `PATH`, the earlier one wins and an upgrade through the other method silently has no effect. This is the most common cause of "I upgraded but the fix isn't there".

## Verifying an install or upgrade

`wtp --version` embeds the build time and commit hash. After any install or upgrade:

1. Run `which wtp` to see which binary is on `PATH`
2. Run `wtp --version` and check the commit hash matches the source you built
3. If the hash is stale, another copy of `wtp` is shadowing the new one — compare `which wtp` against the install destination and update the right file

Do not report an upgrade as done based on the build command succeeding alone.

## Shell integration (`wtp shell-init`)

`wtp cd` cannot change the parent shell's directory by itself. It needs a wrapper function that `wtp shell-init` prints (bash/zsh):

```bash
# add to ~/.zshrc or ~/.bashrc
eval "$(wtp shell-init)"
```

The wrapper intercepts `wtp cd`, points `WTP_DIRECTIVE_FILE` at a temp file, and sources it afterwards. Every other subcommand passes through unchanged.

This is for **human interactive shells only**. Agents should not configure or rely on it for their own execution — use the workspace paths printed by `wtp` commands and change directory directly.

## Shell completions

`wtp completions <shell>` prints a completion script for `zsh`, `bash`, or `fish`:

```bash
# zsh / bash: add to shell config
eval "$(wtp completions zsh)"
```

For fish, write it to the standard completions directory instead of eval-ing.

## Recommended flow

### Install or upgrade wtp

1. Build and install with one method (`cargo install --path wtp-cli` or `./install.sh`)
2. Run `which wtp` and confirm it points at that method's destination
3. Run `wtp --version` and confirm the commit hash is current
4. Report the binary path and version string

### Set up shell integration for a user

1. Detect the user's shell (`echo $SHELL` or ask)
2. Add `eval "$(wtp shell-init)"` to the shell config file (zsh/bash only)
3. Optionally add `eval "$(wtp completions <shell>)"` as well
4. Tell the user to restart the shell or source the config; do not claim `wtp cd` works until a new shell has loaded the wrapper

## Agent output expectations

Report back:

- which install method was used and the destination path
- the `wtp --version` output after the install
- whether `which wtp` matches the freshly installed binary
- what shell config lines were added, if any

## Anti-patterns

Avoid these mistakes:

- reporting an upgrade as successful without checking `wtp --version` afterwards
- installing via one method when the active binary on `PATH` came from the other
- setting up `wtp shell-init` for the agent's own workflow — it only exists to make `wtp cd` work for humans
- eval-ing fish completions in a bash/zsh config
