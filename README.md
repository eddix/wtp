# wtp - WorkTree for Polyrepo

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A CLI tool for managing git worktrees across multiple repositories in a polyrepo workflow.

## Overview

`wtp` helps you manage parallel development across multiple git repositories by leveraging [git worktree](https://git-scm.com/docs/git-worktree). It allows you to:

- Create **workspaces** that group related worktrees from different repositories
- Quickly switch your current repository to any workspace
- View the status of all worktrees in a workspace at a glance
- Use **host aliases** to simplify repository path references

## Installation

### Prerequisites

- Rust 1.90+ (for Rust 2024 edition support)
- Git 2.30+

### From Source

```bash
git clone https://github.com/yourusername/wtp
cd wtp
cargo install --path .
```

## Quick Start

```bash
# Create a new workspace for your feature
wtp create feature-x

# Switch your current repository to the workspace
# This creates a new worktree and branch named "feature-x"
cd ~/projects/my-repo
wtp switch feature-x

# Add another repository to the same workspace
cd ~/projects/another-repo
wtp switch feature-x

# Jump to workspace directory (requires shell integration, see below)
wtp cd feature-x

# Or import an external repo into the current workspace
wtp import ~/projects/another-repo

# See all worktrees in the workspace
wtp status
```

## Configuration

### Config File Locations (Priority Order)

`wtp` searches for configuration in the following order (first existing file wins):

1. `~/.wtp.toml`
2. `~/.wtp/config.toml`
3. `~/.config/wtp/config.toml`

If multiple config files exist, a warning is displayed showing which file is being used.

### Example Configuration

```toml
# Workspace settings
workspace_root = "~/.wtp/workspaces"  # Default location for all workspaces
default_workspace = "main"             # Optional default workspace

# Host aliases - map short names to code roots
[hosts.gh]
root = "~/codes/github.com"

[hosts.gl]
root = "~/codes/gitlab.company.internal"

[hosts.bb]
root = "~/codes/bitbucket.org"

# Default host to use when none specified
default_host = "gh"

# Workspaces are auto-registered, but you can also define them manually
[workspaces]
main = "~/.wtp/workspaces/main"
feature-x = "~/.wtp/workspaces/feature-x"
```

### Hooks

wtp supports running custom scripts on workspace lifecycle events. This is useful for initializing workspaces with standard configurations, tools, or documentation.

#### On-Create Hook

The `on_create` hook runs after a new workspace is created. Configure it in your config file:

```toml
[hooks]
on_create = "~/.wtp/hooks/on-create.sh"
```

The hook script receives these environment variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `WTP_WORKSPACE_NAME` | Name of the created workspace | `my-feature` |
| `WTP_WORKSPACE_PATH` | Full path to the workspace directory | `/home/user/.wtp/workspaces/my-feature` |

**Example hook script** (`~/.wtp/hooks/on-create.sh`):

```bash
#!/bin/bash
echo "Initializing workspace: $WTP_WORKSPACE_NAME"
cd "$WTP_WORKSPACE_PATH"

# Create a README
cat > README.md << EOF
# $WTP_WORKSPACE_NAME

Created: $(date)
EOF

# Copy spec coding config (example)
# cp ~/.templates/spec-coding.toml "$WTP_WORKSPACE_PATH/.spec.toml"

echo "✅ Workspace initialized!"
```

Make the script executable:
```bash
chmod +x ~/.wtp/hooks/on-create.sh
```

**Notes:**
- Use `wtp create <name> --no-hook` to skip the hook
- Hook failures don't block workspace creation (a warning is shown)
- Hook stdout is displayed to the terminal
- On Unix, the script must have execute permissions

## Commands

### `wtp ls` - List Workspaces

```bash
# List all workspaces
wtp ls

# Detailed listing
wtp ls --long
```

Output:
```
main
feature-x
hotfix-123
```

All workspaces are stored under `workspace_root` (default: `~/.wtp/workspaces`).

### `wtp create <NAME>` - Create a New Workspace

```bash
wtp create my-feature

# Skip the on_create hook (if configured)
wtp create my-feature --no-hook
```

Creates a new workspace directory at `<workspace_root>/<NAME>` and registers it in the global config. If an `on_create` hook is configured, it will be executed after the workspace is created.

**Note:** The command outputs the path but cannot change your shell's directory. You'll need to `cd` manually:

```bash
cd $(wtp create my-feature 2>&1 | grep "Created" | awk '{print $NF}')
```

### `wtp rm <NAME>` - Remove a Workspace

```bash
# Remove from config only (safe)
wtp rm my-feature

# Remove from config AND delete directory (⚠️ DANGER)
wtp rm my-feature --delete-dir

# Force deletion without confirmation
wtp rm my-feature --delete-dir --force
```

**⚠️ Warning:** `--delete-dir` permanently deletes the workspace directory and all its contents. Ensure all worktrees are properly committed and pushed first.

**Note:** All workspaces are always stored under `workspace_root` (default: `~/.wtp/workspaces`). This ensures consistent management and prevents fragmentation.

### Security Fence

wtp includes a **fence** mechanism that prevents accidental file operations outside the `workspace_root`. If you attempt to:
- Create a workspace outside `workspace_root`
- Import/switch to a workspace that's outside the boundary

You'll see a warning like:
```
⚠️ Warning: Workspace 'xxx' is outside workspace_root: /Users/you/.wtp/workspaces
Target path: /some/outside/path
Are you sure you want to proceed? [y/N]
```

This protects your system files from accidental modification by wtp commands.

### `wtp import` - Import a Worktree into the Current Workspace

Import an external git repository into the workspace you're currently in.

```bash
# Import a repo into the current workspace (when you're in a workspace directory)
wtp import company/project

# Import using default host (if configured)
wtp import company/project

# Import with explicit workspace
wtp import company/project --workspace my-feature

# Import with explicit host
wtp import company/project --host gh

# Import with full repo path
wtp import --repo ~/projects/my-repo

# Specify branch name (defaults to workspace name)
wtp import company/project --branch feature-xyz

# Specify base for new branch
wtp import company/project --base main
```

**Repository Path Resolution:**

1. If `--repo` is provided: uses the absolute path directly
2. If `--host` is provided: `<host_root>/<path>`
3. If `default_host` is configured: uses that host
4. Otherwise: treats as absolute/relative filesystem path

**Workspace Resolution:**

1. If `--workspace` is provided: uses that workspace
2. If you're in a workspace directory: uses the current workspace
3. Otherwise: error - must specify workspace or be in one

### `wtp switch <WORKSPACE>` - Switch Current Repo to Workspace

Add the current git repository to a workspace by creating a worktree.

```bash
# Switch current repo to an existing workspace
wtp switch my-feature

# Switch and create workspace if it doesn't exist
wtp switch --create my-feature

# With specific branch name
wtp switch my-feature --branch custom-branch

# With specific base
wtp switch my-feature --base develop
```

This command:
1. Detects the current git repository
2. Creates/uses the specified branch
3. Creates a worktree in the target workspace
4. Records the worktree in the workspace's metadata

**Workspace Handling:**
- Without `--create`: workspace must already exist
- With `--create`: creates workspace if it doesn't exist

**Host Matching:** When recording the worktree, `wtp` tries to match the repository path against configured host aliases to store a relative reference instead of an absolute path.

### `wtp host` - Manage Host Aliases

Host aliases map short names to code root directories, making it easier to reference repositories.

```bash
# Add a host alias
wtp host add gh ~/codes/github.com

# List configured hosts
wtp host ls

# Set default host
wtp host set-default gh

# Remove a host alias
wtp host rm gh

# Unset default host
wtp host set-default none
```

**Example workflow:**

```bash
# 1. Add host aliases
wtp host add gh ~/codes/github.com
wtp host add gl ~/codes/gitlab.company.com

# 2. Set default
wtp host set-default gh

# 3. Now you can use short paths
wtp import mycompany/project
# Resolves to: ~/codes/github.com/mycompany/project
```

**Configuration:**

Hosts are stored in `~/.wtp/config.toml`:

```toml
[hosts.gh]
root = "/home/user/codes/github.com"

[hosts.gl]
root = "/home/user/codes/gitlab.company.com"

default_host = "gh"
```

### `wtp status` - Show Workspace Status

Local command - shows status of the current workspace (if you're in one).

```bash
# Show status of current workspace (when in a workspace directory)
wtp status

# Show status of specific workspace
wtp status --workspace my-feature

# Detailed status
wtp status --long

# Include dirty checks (slower)
wtp status --dirty
```

**Note:** If not in a workspace directory, use `--workspace <NAME>` to specify.

Output:
```
REPOSITORY                BRANCH               STATUS
my-project                feature-x            clean
another-project           feature-x            dirty *
```

### `wtp cd <WORKSPACE>` - Change to Workspace Directory

```bash
# Jump to a workspace directory
wtp cd my-feature
```

**Note:** This command requires shell integration. Without it, you'll see:
```
Error: wtp cd requires shell integration
```

### `wtp shell-init` - Generate Shell Integration

To enable `wtp cd`, add the following to your `.zshrc` or `.bashrc`:

```bash
eval "$(wtp shell-init)"
```

Or manually add the wrapper function (see `wtp shell-init` output).

**How it works:**
1. The shell wrapper creates a temporary file and sets `WTP_DIRECTIVE_FILE`
2. `wtp cd` writes a `cd '/path/to/workspace'` command to this file
3. After wtp exits, the wrapper sources this file, changing the parent shell's directory

## Concepts

### Workspace

A **workspace** is a logical collection of worktrees from different repositories. All workspaces are stored under `workspace_root` (default: `~/.wtp/workspaces`).

Each workspace contains:
- `.wtp/worktree.toml` - Metadata about all worktrees
- Subdirectories for each repository's worktrees

### Worktree Layout

Worktrees are organized as:
```
<workspace_root>/<workspace_name>/<repo_slug>/
```

For example:
```
~/.wtp/workspaces/feature-x/
├── my-project/             # worktree for "my-project" repo
└── another-project/        # worktree for "another-project" repo
```

**Constraints:**
- Each repository can only have **one** worktree per workspace
- The worktree directory name is the repository slug (last component of the repo path)
- If you try to add a duplicate, you'll get an error like:
  ```
  Error: Repository 'my-project' is already in this workspace with branch 'feature-x'.
  Each repository can only have one worktree per workspace.
  ```

### Host Aliases

Host aliases let you use short names instead of full paths:

```toml
[hosts.gh]
root = "~/codes/github.com"
```

Now instead of:
```bash
wtp import ~/codes/github.com/company/project
```

You can use:
```bash
wtp import company/project --host gh
# or with default_host configured:
wtp import company/project
```

## Branch Management

### Creating New Branches

By default, `wtp import` and `wtp switch` create branches named after the workspace:

```bash
wtp create feature-x
wtp switch feature-x    # Creates branch "feature-x" from current HEAD
```

### Using Existing Branches

If a branch already exists and is not checked out elsewhere:

```bash
wtp import company/project --branch existing-branch
```

This will check out the existing branch instead of creating a new one.

### ⚠️ Branch Conflicts

Git worktree has a constraint: **a branch can only be checked out in one worktree at a time**.

If you try to add a branch that's already checked out:

```
Error: Branch 'feature-x' is already checked out in another worktree: my-project/feature-x
```

**Workarounds:**
1. Use a different branch name: `wtp import ... --branch feature-x-2`
2. Remove the existing worktree first
3. Use a different workspace

## Troubleshooting

### "Workspace is required"

Commands like `wtp import` and `wtp status` require a workspace:

```
Error: Workspace is required. Use --workspace <NAME> to specify the target workspace.
```

Use `--workspace <name>` to specify which workspace to operate on.

### "Branch already checked out"

See [Branch Conflicts](#branch-conflicts) above.

### Multiple config files warning

```
⚠️  Warning: Multiple config files found: ~/.wtp.toml, ~/.wtp/config.toml. Using ~/.wtp.toml
```

Consolidate your configuration into one file and remove the others.

### Git worktree errors

Common git worktree issues:

1. **"already checked out"** - Branch is in use by another worktree
2. **"is not a valid repository"** - The repository path doesn't exist or isn't a git repo
3. **"is locked"** - A previous git operation was interrupted; may need manual cleanup

## Development

### Building

```bash
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Project Structure

```
src/
├── main.rs              # Entry point
├── cli/                 # CLI subcommands
│   ├── mod.rs
│   ├── ls.rs
│   ├── create.rs
│   ├── host.rs          # Host alias management
│   ├── import.rs
│   ├── remove.rs
│   ├── shell_init.rs
│   ├── status.rs
│   └── switch.rs
├── core/                # Core business logic
│   ├── mod.rs
│   ├── config.rs        # Configuration management
│   ├── error.rs         # Error types
│   ├── fence.rs         # Security fence
│   ├── git.rs           # Git command wrapper
│   ├── workspace.rs     # Workspace management
│   └── worktree.rs      # Worktree data models
```

## Roadmap

- [ ] Fuzzy finder integration (skim)
- [ ] Worktree cleanup/synchronization commands
- [ ] Shell completions
- [ ] Config migration utilities

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
