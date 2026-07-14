---
name: wtp-stacked-worktree
description: Stacked-PR style vertical development with wtp - create stack layers (wtp import --parent), read stack trees in wtp status, cascade-rebase with wtp restack, resolve restack conflicts and resume, retarget layers after a bottom branch merges. Use when working with stacked branches in one repository.
---

# Skill: wtp-stacked-worktree

Use this skill when an agent works with stacked branches (stacked-PR style
development) inside one repository using `wtp`: creating stack layers,
inspecting a stack, cascade-rebasing after a lower layer moved, or rewiring
the stack after a bottom branch merged.

## Use this skill for

- creating a chain of dependent branches, one worktree directory per layer
- reading `wtp status` stack trees and divergence markers
- restacking after a parent layer gained commits
- resolving a restack conflict and resuming
- reparenting a layer after the branch below it merged (squash or not)

## Core model

A stack layer is a normal wtp worktree whose entry in `.wtp/worktree.toml`
carries two extra fields:

- `parent` — the stack edge. Resolved preferentially as the branch of
  another worktree of the same repo in this workspace (forming the tree in
  `wtp status`); otherwise treated as a plain git ref (`origin/main` works)
- `parent_head` — the fork point: the parent commit this layer was cut
  from, updated after every successful restack. `wtp restack` runs
  `git rebase --onto <parent> <parent_head>`, replaying only the layer's
  own commits — this is why a squash-merged bottom layer transplants
  cleanly instead of conflicting

Layer directories are named `<repo_slug>@<branch>`. wtp never touches the
forge: no PR creation, no PR retargeting, no pushes.

## Primary commands

- `wtp import -b <new-branch> --parent <ref>` — create a layer
- `wtp status` — tree view with `↑`/`↓` divergence per layer
- `wtp restack` — cascade rebase (chain scope inside a layer directory,
  all chains from the workspace root)
- `wtp retarget [<worktree-dir>] <new-parent>` — rewire an edge
  (metadata only; follow with `wtp restack`)

## Creating layers

1. Work in the parent layer's worktree directory
2. Run `wtp import -b <new-branch> --parent <parent-branch>` — PATH is
   inferred from the directory you stand in; `--with-branch-name` is
   implied; `--base` is rejected (a layer always starts at its parent)
3. Continue work in the printed new worktree directory

The parent ref must exist. `--parent` never falls through to the
interactive repo picker; outside a worktree directory pass the repo
explicitly.

## Restacking

1. Ensure every layer is committed (preflight refuses dirty layers,
   rebases in progress, and unresolvable parents — all listed at once)
2. Run `wtp restack` from the layer's directory (that chain) or the
   workspace root (all chains)
3. Read the summary: `N rebased, M already up to date`
4. Relay the printed `git push --force-with-lease` checklist to the user —
   wtp does not push, and the agent should not push without being asked

## Conflict resolution loop (agent workflow)

`wtp restack` is stateless and idempotent — there is no `--continue`. On
conflict it stops and prints the worktree path, the conflicted files, and
resume instructions. The loop is:

1. `cd` into the printed worktree directory
2. Resolve the conflicted files (they are listed; also `git status`)
3. `git add` the resolutions, then `git rebase --continue`
   (set `GIT_EDITOR=true` to avoid an editor prompt)
4. Re-run `wtp restack` — finished layers are detected as up to date
   (their fork point is healed automatically) and the run continues with
   the remaining layers

Never run `git rebase --abort` unless the user asked to abandon the
restack.

## After the bottom of a stack merges

Typical flow when `feat-1` (bottom) was merged into `main` and deleted:

1. `wtp eject <feat-1-dir>` — removing the worktree does NOT break the
   chain (children resolve the parent as a plain ref while the branch
   exists; after deletion, `wtp status` shows `parent missing`)
2. `wtp retarget <feat-2-dir> main` — rewires the edge; the old fork
   point is intentionally preserved
3. `wtp restack` — transplants only `feat-2`'s own commits onto `main`;
   clean even when the merge was a squash

## Safety rules

1. Do not push rewritten branches without explicit user intent; surface
   the force-push checklist instead.
2. Treat a preflight failure as a report, not an obstacle to bypass —
   list the offending layers to the user.
3. Retarget rejects self-parenting, unknown refs, and cycles; surface the
   error rather than editing worktree.toml by hand.
4. An interrupted rebase belongs to the layer's own worktree — resolve it
   there; other layers are untouched.

## Agent output expectations

Report back:

- the stack shape (which branches, in what order) after the operation
- for restack: how many layers rebased vs skipped, and the force-push
  checklist
- for conflicts: which layer, which files, and what resolution was applied
- for retarget: old parent -> new parent, and whether restack was run

## Anti-patterns

Avoid these mistakes:

- running `git rebase` manually across layers instead of `wtp restack`
  (loses fork-point maintenance)
- editing `parent`/`parent_head` in worktree.toml by hand
- force-pushing without being asked
- using `--base` together with `--parent`
- treating `parent missing` as an error to work around — it is a prompt
  to `wtp retarget`
