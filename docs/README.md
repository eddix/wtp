# Documentation

User-facing entry points (`README.md`, `INSTALL.md`, `AGENTS.md`) live in the repo root. Everything else — design notes, review reports, integration guides — lives here.

## Layout

- [`agent-integration/`](agent-integration/) — How to expose wtp's agent-facing skills to Codex / Claude Code / Cursor.
- [`design/`](design/) — Forward-looking design documents (what to build, why, how).
- [`reviews/`](reviews/) — Backward-looking review and audit reports (what shipped, how it scored).

## Design ↔ Review pairs

Each design document has a corresponding review with the same filename, so it's easy to compare intent against outcome.

| Topic | Design | Review |
|---|---|---|
| Cargo workspace restructure | [`design/workspace-restructure.md`](design/workspace-restructure.md) | [`reviews/workspace-restructure.md`](reviews/workspace-restructure.md) |
| wtp-gui implementation | [`design/gui-implementation.md`](design/gui-implementation.md) | [`reviews/gui-implementation.md`](reviews/gui-implementation.md) |
| Audit fixes (security / perf / quality) | [`design/audit-fixes.md`](design/audit-fixes.md) | [`reviews/audit-fixes.md`](reviews/audit-fixes.md) |
| Stacked worktree (纵向开发) | [`design/stacked-worktree.md`](design/stacked-worktree.md) | — |

## Standalone audit reports

- [`reviews/audit-report.md`](reviews/audit-report.md) — Unified 3-dimension audit (security + performance + code quality) of the workspace restructure.
- [`reviews/audit-code-quality.md`](reviews/audit-code-quality.md) — Detailed code-quality findings backing the unified report.
