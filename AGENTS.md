# wtp (WorkTree for Polyrepo) - Agent Context

## Project Overview
A CLI tool for managing git worktrees across multiple repositories in a polyrepo workflow.

## Safety Rules for AI Agents

### File Deletion Safety
**CRITICAL**: Before executing any `rm -rf` or file deletion operations, you **MUST** get explicit user confirmation.

**Instead of:**
```bash
rm -rf ~/.wtp/workspaces/my-ws
```

**Use:**
```bash
./scripts/safe-rm.sh ~/.wtp/workspaces/my-ws
```

Or manually confirm:
```bash
echo "Will delete: ~/.wtp/workspaces/my-ws"
ls -la ~/.wtp/workspaces/my-ws
read -p "Confirm? [yes/N] " confirm
[ "$confirm" = "yes" ] && rm -rf ~/.wtp/workspaces/my-ws
```

**Rules:**
1. **Always list first**: Show what will be deleted
2. **Get explicit confirmation**: Wait for user to type "yes"
3. **Prefer safe-rm.sh**: Use the provided script when possible
4. **Never use `rm -rf` directly** without confirmation

### Recovery (if data is accidentally deleted)
1. Check macOS Trash: `~/.Trash/`
2. Check if Time Machine has backups
3. Contact system administrator

---

## Module Structure

```
src/
├── main.rs              # Entry point, error styling
├── cli/                 # CLI subcommands
│   ├── mod.rs           # CLI argument definitions (clap), command routing
│   ├── cd.rs            # Change to workspace directory (needs shell integration)
│   ├── create.rs        # Create new workspace (--no-hook flag supported)
│   ├── host.rs          # Manage host aliases (add, ls, rm, set-default)
│   ├── import.rs        # Import a worktree into workspace
│   ├── ls.rs            # List workspaces (--short, --long flags)
│   ├── remove.rs        # Remove workspace (--delete-dir, --force flags)
│   ├── shell_init.rs    # Generate shell integration script
│   ├── status.rs        # Show workspace status (--workspace, --long, --dirty flags)
│   └── switch.rs        # Switch current repo to workspace (--create, --branch, --base flags)
└── core/                # Core business logic (UI-independent)
    ├── mod.rs           # Module exports
    ├── config.rs        # GlobalConfig, WorkspaceConfig, HostConfig, HooksConfig
    ├── error.rs         # WtpError enum, Result type
    ├── fence.rs         # Security fence for file operations
    ├── git.rs           # GitClient wrapper around git CLI
    ├── workspace.rs     # WorkspaceManager (create_workspace, remove_workspace, etc.)
    └── worktree.rs      # WorktreeEntry, WorktreeManager, RepoRef, WorktreeToml
```

---

## Commands (Flat Structure)

Commands are grouped visually in help output but kept flat (not nested).

### Workspace Management
- `wtp cd <workspace>` - Change to workspace dir (requires shell integration via `WTP_DIRECTIVE_FILE`)
- `wtp create <name> [--no-hook]` - Create new workspace, optionally skip on_create hook
- `wtp ls [--short|--long]` - List workspaces
- `wtp rm <name> [--delete-dir] [--force]` - Remove workspace
- `wtp status [--workspace <name>] [--long] [--dirty]` - Show workspace status

### Repository Operations
- `wtp import [path] [--workspace <name>] [--host <alias>] [--repo <path>] [--branch <name>] [--base <ref>]` - Import repo worktree into workspace
- `wtp switch <workspace> [--create] [--branch <name>] [--base <ref>]` - Switch current repo to workspace

### Utilities
- `wtp host <add|ls|rm|set-default>` - Manage host aliases
- `wtp shell-init` - Generate shell integration script

---

## Key Data Structures

### GlobalConfig (src/core/config.rs)
```rust
pub struct GlobalConfig {
    pub workspace_root: PathBuf,           // default: ~/.wtp/workspaces
    pub workspaces: IndexMap<String, PathBuf>, // name -> path mapping
    pub hosts: HashMap<String, HostConfig>,
    pub default_host: Option<String>,
    pub hooks: HooksConfig,                // on_create hook path
}
```

### HooksConfig (src/core/config.rs)
```rust
pub struct HooksConfig {
    pub on_create: Option<PathBuf>, // Script run after workspace creation
}
```

Environment variables passed to on_create hook:
- `WTP_WORKSPACE_NAME` - Name of created workspace
- `WTP_WORKSPACE_PATH` - Full path to workspace directory

Hook behavior:
- Hook failures don't block workspace creation (warning shown)
- Hook stdout is printed to terminal
- On Unix, script must have execute permissions

### WorktreeEntry (src/core/worktree.rs)
```rust
pub struct WorktreeEntry {
    pub id: WorktreeId,
    pub repo: RepoRef,              // Hosted or Absolute
    pub branch: String,
    pub worktree_path: PathBuf,     // Relative to workspace root
    pub base: Option<String>,       // Base ref for branch creation
    pub head_commit: Option<String>,
}
```

### RepoRef (src/core/worktree.rs)
```rust
pub enum RepoRef {
    Hosted { host: String, path: String },  // e.g., gh:abc/def
    Absolute { path: PathBuf },             // e.g., /home/user/repo
}
```

---

## Directory Layout

### Workspace Root Layout
```
<workspace_root>/<workspace_name>/
├── .wtp/
│   ├── worktree.toml    # Metadata about worktrees
│   └── config.toml      # Per-workspace config (optional)
└── <repo_slug>/         # Worktree directories (flat, no branch subdirs)
```

- Flat structure (no branch subdirectories)
- One worktree per repository per workspace limit enforced

### Config File Locations (Priority Order)
1. `~/.wtp.toml`
2. `~/.wtp/config.toml`
3. `~/.config/wtp/config.toml`

First existing file wins. Multiple config files trigger a warning.

---

## Security Fence (src/core/fence.rs)

Prevents file operations outside `workspace_root` without explicit confirmation.

### Implementation
```rust
pub struct Fence {
    boundary: PathBuf,      // The workspace_root
    interactive: bool,      // Whether to prompt for confirmation
}
```

### Methods
- `is_within_boundary(path)` - Check if path is within workspace_root
- `create_dir_all(path)` - Create directory with fence check
- `write(path, content)` - Write file with fence check
- `remove_dir_all(path)` - Remove directory with fence check
- `remove_file(path)` - Remove file with fence check

### Global Instance
Initialized at startup in `cli/mod.rs`:
```rust
crate::core::fence::init_global_fence(global_config.workspace_root.clone());
```

### User Experience
When an operation targets a path outside `workspace_root`:
```
⚠️  SECURITY WARNING
 Operation: create directory
 Target: /some/outside/path
 This is OUTSIDE the workspace_root: /Users/you/.wtp/workspaces
 
 Are you sure you want to proceed? [y/N] 
```

---

## Shell Integration

### Purpose
Enable `wtp cd` to change the parent shell's directory. A child process cannot directly modify the parent's working directory.

### Mechanism: WTP_DIRECTIVE_FILE
1. Shell wrapper (`wtp shell-init`) creates a temp file and sets `WTP_DIRECTIVE_FILE`
2. `wtp cd` writes `cd '/path/to/workspace'` to this file
3. After wtp exits, wrapper sources the file, executing cd in parent shell

### Shell Wrapper Script (bash/zsh)
```bash
wtp() {
    local tmpfile=""
    
    if [[ "$1" == "cd" ]]; then
        tmpfile=$(mktemp "${TMPDIR:-/tmp}/wtp.XXXXXX")
        export WTP_DIRECTIVE_FILE="$tmpfile"
    fi
    
    command wtp "$@"
    local exit_code=$?
    
    if [[ -n "$tmpfile" && -s "$tmpfile" ]]; then
        source "$tmpfile"
        rm -f "$tmpfile"
        unset WTP_DIRECTIVE_FILE
    elif [[ -n "$tmpfile" ]]; then
        rm -f "$tmpfile"
        unset WTP_DIRECTIVE_FILE
    fi
    
    return $exit_code
}
```

### Setup
```bash
eval "$(wtp shell-init)"
```

---

## Error Handling

### WtpError Variants (src/core/error.rs)
- `Io` - std::io::Error
- `Config(String)` - Configuration errors
- `Git(String)` - Git operation errors
- `WorkspaceNotFound { name }`
- `WorkspaceAlreadyExists { name, path }`
- `NotInWorkspace { message }`
- `NotInGitRepo`
- `RepoNotFound { path }`
- `BranchAlreadyCheckedOut { branch, worktree_path }`
- `WorktreeAlreadyExists { path }`
- `HostNotFound { alias }`
- `Parse(String)`
- `Serialization` / `Deserialization` - TOML errors
- `MultipleConfigFiles { files, used }`

### Error Display
Errors are displayed in red using `anstyle` in `main.rs`:
```rust
let error_style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));
eprintln!("{error_style}Error:{error_style:#} {e}");
```

---

## Technical Stack
- Rust 2024 edition
- clap 4.5 with derive features
- colored, anstyle/anstream for colors
- shellexpand for path expansion
- tokio for async runtime
- chrono for timestamps
- serde + toml for serialization
- indexmap for ordered workspace map
- uuid for worktree IDs

---

## Testing Strategy
- **Unit tests**: In-module tests for core logic (see `src/core/fence.rs` tests)
- **Integration tests**: CLI commands with temporary directories and isolated HOME
- **Test isolation**: Tests must use temp directories as HOME to avoid polluting user's `~/.wtp`

### Running Tests
```bash
cargo test
```

---

## Important Notes
- `add` command was renamed to `import`
- Nested subcommand structure was reverted to flat commands
- TUI mode was removed (ratatui dependency remains but unused)
- Error messages are displayed in red
- All commands use green color for command names in help output

