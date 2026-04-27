# FIX_DESIGN.md — Audit 修复设计文档

**日期**: 2026-03-28
**范围**: wtp-core + wtp-cli（跳过 wtp-gui S7/S8/S9/W20-W24/A20-A23）

---

## 一、安全高危 (H1-H3)

### H1. fence.rs Symlink TOCTOU 可绕过 fence 边界

**当前代码位置**: `wtp-core/src/fence.rs:126-133`

```rust
// fence.rs:126-133
pub fn create_dir_all(&self, path: &Path) -> Result<()> {
    let within = self.check_path(path, "create directory")?;
    std::fs::create_dir_all(path)?;
    // Re-verify after creation to catch symlink races (only for in-boundary paths)
    if within {
        self.verify_canonical(path, "create directory")?;
    }
    Ok(())
}
```

**问题**: `std::fs::create_dir_all` 会跟随 symlink，在 boundary 外创建目录。创建后才 `verify_canonical` 存在 TOCTOU 窗口。

**修复方案**:

1. 新增辅助函数 `safe_create_dir_all`，逐级创建目录，每一级检查 symlink：

```rust
/// Safely create directories one level at a time, refusing to follow symlinks.
fn safe_create_dir_all(&self, path: &Path) -> Result<()> {
    let canonical_boundary = self.boundary.canonicalize()
        .unwrap_or_else(|_| self.boundary.clone());

    // Find the deepest existing ancestor
    let mut ancestors: Vec<&Path> = Vec::new();
    let mut current = path;
    while !current.exists() {
        ancestors.push(current);
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    // Verify existing ancestor is not a symlink outside boundary
    if current.exists() {
        let meta = std::fs::symlink_metadata(current)?;
        if meta.is_symlink() {
            return Err(WtpError::config(format!(
                "Security: ancestor path is a symlink: {}",
                current.display()
            )));
        }
        let canonical = current.canonicalize()?;
        if !canonical.starts_with(&canonical_boundary) {
            return Err(WtpError::config(format!(
                "Security: ancestor resolves outside boundary: {}",
                canonical.display()
            )));
        }
    }

    // Create each level, checking for symlink after each mkdir
    for dir in ancestors.into_iter().rev() {
        std::fs::create_dir(dir)?;
        // Immediately verify no symlink race
        let meta = std::fs::symlink_metadata(dir)?;
        if meta.is_symlink() {
            // Someone swapped the directory for a symlink — remove and error
            let _ = std::fs::remove_dir(dir);
            return Err(WtpError::config(format!(
                "Security: directory replaced by symlink during creation: {}",
                dir.display()
            )));
        }
    }
    Ok(())
}
```

2. 修改 `create_dir_all` 调用 `safe_create_dir_all` 替代 `std::fs::create_dir_all`。

3. 在 `is_within_boundary` (fence.rs:55-79) 中添加 symlink 检查：对已存在路径的所有祖先检查 `symlink_metadata`。

**影响范围**: `wtp-core/src/fence.rs` 仅此一个文件。

---

### H2. eject 路径遍历

**当前代码位置**: `wtp-cli/src/cli/eject.rs:62`

```rust
// eject.rs:62
let worktree_path_abs = workspace_path.join(&worktree_path_rel);
```

**问题**: `worktree_path_rel` 直接从 `.wtp/worktree.toml` 读取，若被篡改为 `../../victim`，则 `worktree_path_abs` 指向 workspace 外。随后在 78 行调用 `git.get_status` 和 97 行调用 `git.remove_worktree` 将操作外部 worktree。

**修复方案**:

在 `eject.rs:62` 之后添加边界校验：

```rust
let worktree_path_abs = workspace_path.join(&worktree_path_rel);

// H2 fix: validate worktree path is within workspace
let canonical_workspace = workspace_path.canonicalize()?;
let canonical_worktree = worktree_path_abs.canonicalize()
    .unwrap_or_else(|_| {
        // Path doesn't exist yet, use lexical normalization
        wtp_core::fence::lexical_normalize(&worktree_path_abs)
    });
if !canonical_worktree.starts_with(&canonical_workspace) {
    anyhow::bail!(
        "Security: worktree path '{}' resolves outside workspace boundary '{}'",
        worktree_path_rel.display(),
        workspace_path.display()
    );
}
```

**但**: `lexical_normalize` 是 `fence.rs` 中的私有函数。需要在 `wtp-core` 中暴露一个公共 path 验证 API。

**实际方案**: 在 `wtp-core/src/fence.rs` 新增公共函数：

```rust
/// Validate that `child` is within `parent` boundary after canonicalization.
/// Returns the canonical child path, or an error if outside boundary.
pub fn validate_within_boundary(parent: &Path, child: &Path) -> Result<PathBuf> {
    let canonical_parent = parent.canonicalize()
        .map_err(|e| WtpError::config(format!("Cannot canonicalize parent: {}", e)))?;

    let canonical_child = if child.exists() {
        child.canonicalize()
            .map_err(|e| WtpError::config(format!("Cannot canonicalize child: {}", e)))?
    } else {
        lexical_normalize(child)
    };

    if !canonical_child.starts_with(&canonical_parent) {
        return Err(WtpError::config(format!(
            "Path traversal blocked: '{}' resolves outside '{}'",
            child.display(),
            parent.display()
        )));
    }
    Ok(canonical_child)
}
```

然后在 `eject.rs:62` 后：

```rust
wtp_core::fence::validate_within_boundary(&workspace_path, &worktree_path_abs)?;
```

**影响范围**:
- `wtp-core/src/fence.rs` — 新增 `validate_within_boundary` 公共函数，公开 `lexical_normalize`（或复制逻辑）
- `wtp-cli/src/cli/eject.rs:62` — 添加校验调用

---

### H3. remove 路径遍历

**当前代码位置**: `wtp-cli/src/cli/remove.rs:45,73`

```rust
// remove.rs:45
let wt_path = workspace_path.join(&entry.worktree_path);

// remove.rs:73
let wt_path = workspace_path.join(&entry.worktree_path);
```

**问题**: 与 H2 同源，批量循环中 `entry.worktree_path` 可能被篡改。

**修复方案**: 在两处 `workspace_path.join()` 后立即校验：

```rust
let wt_path = workspace_path.join(&entry.worktree_path);
wtp_core::fence::validate_within_boundary(&workspace_path, &wt_path)?;
```

**影响范围**:
- `wtp-cli/src/cli/remove.rs:45,73` — 添加校验
- 复用 H2 中新增的 `validate_within_boundary`

---

## 二、性能 Critical (P1)

### P1. status --long 每仓库 5-6 次串行 git fork

**当前代码位置**: `wtp-cli/src/cli/status.rs:105-227`

```rust
// status.rs:140  get_ahead_behind
// status.rs:163  get_head_commit
// status.rs:164-165  get_last_commit_subject
// status.rs:167-168  get_last_commit_relative_time
// status.rs:182  get_status
// status.rs:207  get_stash_count
```

**问题**: 每个 worktree 串行执行 6 次 git 子进程调用。

**修复方案**:

1. 在 `wtp-core/src/git.rs` 新增合并查询方法：

```rust
/// Combined HEAD info: (short_hash, subject, relative_time)
pub fn get_head_info(&self, repo_path: &Path) -> Result<(String, String, String)> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .arg("log")
        .arg("-1")
        .arg("--format=%h\x00%s\x00%cr")
        .output()?;

    if !output.status.success() {
        return Err(WtpError::git("Failed to get HEAD info"));
    }

    let out = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = out.trim().splitn(3, '\0').collect();
    if parts.len() == 3 {
        Ok((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
    } else {
        Ok((String::new(), String::new(), String::new()))
    }
}

/// Combined status with stash: parses `git status --porcelain=v2 --branch --show-stash`
pub fn get_full_status(&self, repo_path: &Path) -> Result<FullGitStatus> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .arg("status")
        .arg("--porcelain=v2")
        .arg("--branch")
        .arg("--show-stash")
        .output()?;
    // ... parse v2 format including stash count
}
```

2. 在 `status.rs` 中 `print_detailed_status` 将 6 次调用合并为 2 次（`get_head_info` + `get_full_status`），加上 `get_ahead_behind` 共 3 次。

3. **暂不做跨仓库并行**（涉及 async 改动太大，留 P2 一起做）。

**影响范围**:
- `wtp-core/src/git.rs` — 新增 `get_head_info`、`get_full_status` 方法及 `FullGitStatus` 结构体
- `wtp-cli/src/cli/status.rs:105-227` — 重写 `print_detailed_status` 使用合并方法

---

## 三、代码严重 (S1-S6)

### S1. Core 库直接读写 stdin/stderr

**当前代码位置**: `wtp-core/src/fence.rs:102-107`

```rust
// fence.rs:102-107
if self.interactive {
    eprintln!("{}", prompt);
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
```

**问题**: 核心库直接与终端交互，阻止 GUI 和测试使用。

**修复方案**: 使用回调 trait 替代直接 I/O：

1. 在 `wtp-core/src/fence.rs` 中定义：

```rust
/// Callback for fence boundary violations requiring user confirmation.
pub trait FenceConfirm: Send + Sync {
    /// Prompt user for confirmation. Returns true if user approves.
    fn confirm(&self, prompt: &str) -> Result<bool>;
}

/// Default implementation that reads from stdin/stderr (CLI use)
pub struct StdioConfirm;

impl FenceConfirm for StdioConfirm {
    fn confirm(&self, prompt: &str) -> Result<bool> {
        eprintln!("{}", prompt);
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().eq_ignore_ascii_case("y"))
    }
}
```

2. 修改 `Fence` 结构体：

```rust
pub struct Fence {
    boundary: PathBuf,
    confirm: Option<Box<dyn FenceConfirm>>,
}
```

3. 修改 `check_path` 中 `self.interactive` 分支为调用 `self.confirm`。

4. `StdioConfirm` 由 CLI 层注入（`wtp-cli/src/cli/mod.rs:370`），core 默认不带 confirm（即非交互模式）。

**影响范围**:
- `wtp-core/src/fence.rs` — 新增 trait，修改 `Fence` struct 和 `check_path`
- `wtp-cli/src/cli/mod.rs:370` — 注入 `StdioConfirm`

---

### S2. 库代码中 println!/eprintln!

**当前代码位置**: `wtp-core/src/workspace.rs:77,125`

```rust
// workspace.rs:77
eprintln!("Warning: Failed to run create hook: {}", e);

// workspace.rs:125
println!("{}", stdout);
```

**修复方案**:

1. 修改 `create_workspace` 返回值为包含 hook 输出的结构体：

```rust
pub struct CreateResult {
    pub path: PathBuf,
    pub hook_output: Option<String>,
    pub hook_warning: Option<String>,
}
```

2. 将 `eprintln!` 改为设置 `hook_warning` 字段。
3. 将 `println!("{}", stdout)` 改为设置 `hook_output` 字段。
4. 在 CLI 层（`wtp-cli/src/cli/create.rs`）打印这些信息。

**影响范围**:
- `wtp-core/src/workspace.rs:51-82,118-126` — 返回结构体替代打印
- `wtp-cli/src/cli/create.rs` — 处理返回值并打印
- `wtp-cli/src/cli/switch.rs:74,94` — 这两处也调用 `create_workspace`，同步修改

---

### S3. RepoRef::to_absolute_path 类型签名不匹配

**当前代码位置**: `wtp-core/src/worktree.rs:48`

```rust
// worktree.rs:48
pub fn to_absolute_path(&self, hosts: &std::collections::HashMap<String, PathBuf>) -> PathBuf {
```

**问题**: 配置实际存储 `HashMap<String, HostConfig>` 但此函数接受 `HashMap<String, PathBuf>`。每个调用方（如 `import.rs:93-98`）必须手动提取 `.root`。

**修复方案**:

修改签名直接接受 `HostConfig`：

```rust
pub fn to_absolute_path(&self, hosts: &std::collections::HashMap<String, crate::config::HostConfig>) -> PathBuf {
    match self {
        RepoRef::Hosted { host, path } => {
            if let Some(host_config) = hosts.get(host) {
                host_config.root.join(path)
            } else {
                tracing::warn!("Host '{}' not found, treating path as relative", host);
                PathBuf::from(path)
            }
        }
        RepoRef::Absolute { path } => path.clone(),
    }
}
```

调用方变更（删除手动 map）：

```rust
// import.rs:93-98 BEFORE:
let hosts: std::collections::HashMap<String, PathBuf> = manager
    .global_config()
    .hosts
    .iter()
    .map(|(k, v)| (k.clone(), v.root.clone()))
    .collect();
let repo_path = repo_ref.to_absolute_path(&hosts);

// AFTER:
let repo_path = repo_ref.to_absolute_path(&manager.global_config().hosts);
```

**影响范围**:
- `wtp-core/src/worktree.rs:48-60` — 修改签名
- `wtp-cli/src/cli/import.rs:93-99` — 简化调用
- 搜索所有 `to_absolute_path` 调用方并修改

---

### S4. fence.rs TOCTOU 竞态

与 H1 同源，已在 H1 中修复。

---

### S5. 分支基准回退静默丢失上下文

**当前代码位置**: `wtp-cli/src/cli/switch.rs:169-173` 和 `wtp-cli/src/cli/import.rs:120-125`

```rust
// switch.rs:169-173
let base = args.base.unwrap_or_else(|| {
    git.get_current_branch(&current_repo_root)
        .unwrap_or_else(|_| "HEAD".to_string())
});

// import.rs:120-125
let base = args.base.unwrap_or_else(|| {
    git.get_current_branch(&repo_root)
        .unwrap_or_else(|_| "HEAD".to_string())
});
```

**问题**: 分支获取失败时静默回退到 `"HEAD"`，用户不知道。对 bare repo 来说 HEAD 解析可能出人意料。

**修复方案**: 添加 `tracing::warn!` 和 stderr 提示：

```rust
let base = args.base.unwrap_or_else(|| {
    match git.get_current_branch(&repo_root) {
        Ok(branch) => branch,
        Err(e) => {
            tracing::warn!("Could not detect current branch, falling back to HEAD: {}", e);
            eprintln!(
                "{} Could not detect current branch ({}), using HEAD as base.",
                "Warning:".yellow().bold(),
                e
            );
            "HEAD".to_string()
        }
    }
});
```

**影响范围**:
- `wtp-cli/src/cli/switch.rs:169-173`
- `wtp-cli/src/cli/import.rs:120-125`

---

### S6. macOS grep -oP 不可用

**当前代码位置**: `wtp-cli/src/cli/completions.rs:164,241`

```rust
// completions.rs:164 (in zsh completion function _wtp_hosts)
hosts=(${(f)"$(command wtp host ls 2>/dev/null | grep -oP '^\s+\K\S+(?=\s+->)')"})

// completions.rs:241 (in bash completion function)
hosts="$(command wtp host ls 2>/dev/null | grep -oP '^\s+\K\S+(?=\s+->)')"
```

**问题**: `grep -oP` 使用 PCRE（Perl regex），macOS 默认的 BSD grep 不支持 `-P` 标志。

**修复方案**: 替换为 `awk`：

```bash
# zsh (line 164):
hosts=(${(f)"$(command wtp host ls 2>/dev/null | awk '/->/ {print $1}')"})

# bash (line 241):
hosts="$(command wtp host ls 2>/dev/null | awk '/->/ {print $1}')"
```

**影响范围**: `wtp-cli/src/cli/completions.rs:164,241` — 两处字符串替换。

---

## 四、安全中危 (M1-M4)

### M1. 相对路径 hook 执行

**当前代码位置**: `wtp-core/src/config.rs:50-55` (tilde expansion) + `wtp-core/src/workspace.rs:85-95`

```rust
// config.rs:51-55
if let Some(ref mut hook_path) = cfg.hooks.on_create {
    *hook_path = shellexpand::tilde(&hook_path.to_string_lossy())
        .to_string()
        .into();
}

// workspace.rs:90-91
if !hook_path.exists() {
    return Err(WtpError::config(format!(...)));
```

**问题**: 配置中 `on_create = "./setup.sh"` 会从调用者 CWD 解析，可能执行恶意脚本。

**修复方案**: 在 config 加载时，将相对路径解析为相对于配置文件目录：

```rust
// config.rs, inside LoadedConfig::load(), after tilde expansion:
if let Some(ref mut hook_path) = cfg.hooks.on_create {
    *hook_path = shellexpand::tilde(&hook_path.to_string_lossy())
        .to_string()
        .into();
    // M1 fix: resolve relative hook paths against config file directory
    if hook_path.is_relative() {
        if let Some(config_dir) = path.parent() {
            *hook_path = config_dir.join(&hook_path);
        }
    }
}
```

**影响范围**: `wtp-core/src/config.rs:51-55` — 在 tilde 展开后添加相对路径解析。

---

### M2. Git 参数注入

**当前代码位置**: `wtp-core/src/git.rs:165,205,352`

```rust
// git.rs:165-172 (create_worktree_with_branch)
.arg("-b")
.arg(branch)
.arg(worktree_path)
.arg(base)     // ← user-controlled, no "--" separator

// git.rs:205-211 (add_worktree_for_branch)
.arg(worktree_path)
.arg(branch)   // ← user-controlled

// git.rs:352-359 (remove_worktree)
cmd.arg(worktree_path);  // ← from worktree.toml
```

以及 `git.rs:377-384` (get_ahead_behind):
```rust
.arg(format!("HEAD...{}", base))  // ← user-controlled base
```

**问题**: 虽然 `Command::arg()` 不做 shell 展开，但 Git 自身会将 `-` 开头的值当选项解析。例如 `branch = "--force"` 可能干扰 git 命令。

**修复方案**:

1. 新增验证函数：

```rust
// git.rs, new helper:
/// Reject values that look like git options (start with "-")
fn validate_git_ref(value: &str, param_name: &str) -> Result<()> {
    if value.starts_with('-') {
        return Err(WtpError::git(format!(
            "Invalid {}: '{}' (must not start with '-')",
            param_name, value
        )));
    }
    Ok(())
}
```

2. 在 `create_worktree_with_branch` 的 branch/base 参数前调用：

```rust
validate_git_ref(branch, "branch name")?;
validate_git_ref(base, "base reference")?;
```

3. 在 `add_worktree_for_branch` 的 branch 参数前调用。

4. 在 `get_ahead_behind` 的 base 参数前调用。

5. 在 `remove_worktree` 的 worktree_path 参数前加 `--`：

```rust
cmd.arg("--").arg(worktree_path);
```

**影响范围**: `wtp-core/src/git.rs` — 新增 `validate_git_ref`，修改 4 个方法。

---

### M3. Host 路径遍历 + Git 参数注入 (import)

**当前代码位置**: `wtp-cli/src/cli/import.rs:84-86,117-125`

```rust
// import.rs:84-86 (resolve_repo_ref for hosted mode)
Ok(RepoRef::Hosted {
    host: host_alias.to_string(),
    path: path.to_string(),  // ← user input, no validation
})
```

**问题**: hosted 模式下 `path = "../../other/repo"` 会在 `to_absolute_path` 中拼接到 host_root 后逃逸。

**修复方案**:

1. 在 `import.rs` 的 `resolve_repo_ref` 中添加路径验证：

```rust
fn resolve_repo_ref(...) -> anyhow::Result<RepoRef> {
    if let Some(host_alias) = host {
        let host_root = manager
            .global_config()
            .get_host_root(host_alias)
            .ok_or_else(|| anyhow::anyhow!("Host alias '{}' not found", host_alias))?;

        // M3 fix: validate path doesn't escape host root
        let full_path = host_root.join(path);
        wtp_core::fence::validate_within_boundary(host_root, &full_path)
            .map_err(|e| anyhow::anyhow!("Path traversal blocked: {}", e))?;

        Ok(RepoRef::Hosted {
            host: host_alias.to_string(),
            path: path.to_string(),
        })
    }
    // ...
}
```

2. branch/base 的参数注入由 M2 的 `validate_git_ref` 统一覆盖。

**影响范围**:
- `wtp-cli/src/cli/import.rs:234-244` — 添加路径遍历校验
- 复用 H2 的 `validate_within_boundary` 和 M2 的 `validate_git_ref`

---

### M4. switch.rs Git 参数注入

**当前代码位置**: `wtp-cli/src/cli/switch.rs:166-173`

```rust
// switch.rs:166
let branch = args.branch.unwrap_or_else(|| workspace_name.clone());
// switch.rs:169-173
let base = args.base.unwrap_or_else(|| { ... });
```

**问题**: branch/base 直接传给 git 命令。

**修复方案**: 由 M2 统一覆盖。`git.create_worktree_with_branch` 和 `git.add_worktree_for_branch` 内部已添加 `validate_git_ref`，所有调用方自动受益。无需额外修改 switch.rs。

**影响范围**: 无额外修改（M2 已覆盖）。

---

## 五、性能 High (P2-P4)

### P2. GitClient 全同步阻塞

**当前代码位置**: `wtp-core/src/git.rs:20-421` — 所有方法使用 `std::process::Command::output()`

**问题**: 在 `#[tokio::main]` 下阻塞 tokio worker thread。

**修复方案**: 本轮只做最小改动 — 将核心热路径方法包装 `spawn_blocking`：

```rust
// 在 wtp-core/src/git.rs 新增:
use tokio::task::spawn_blocking;

impl GitClient {
    /// Async wrapper that runs git command in blocking thread pool
    pub async fn get_status_async(&self, repo_path: PathBuf) -> Result<GitStatus> {
        let client = self.clone();
        spawn_blocking(move || client.get_status(&repo_path))
            .await
            .map_err(|e| WtpError::git(format!("Task join error: {}", e)))?
    }
    // 类似地包装 get_head_info, get_ahead_behind, get_stash_count
}
```

**注意**: 这是渐进式改进。全面 async 化（`tokio::process::Command`）作为后续优化。

**影响范围**:
- `wtp-core/src/git.rs` — 新增 `_async` 方法系列
- `wtp-cli/src/cli/status.rs` — 改用 async 方法
- `wtp-cli/src/cli/ls.rs` — 改用 async 方法

---

### P3. ls --long 跨所有 workspace 串行探测

**当前代码位置**: `wtp-cli/src/cli/ls.rs:44-118`

```rust
// ls.rs:44-118 — for (i, ws) in workspaces.iter().enumerate() { ... git calls ... }
```

**修复方案**: 在 P2 提供 async 方法后，用 `futures::stream::iter(...).buffer_unordered(4)` 并行查询各 workspace 的 status：

```rust
use futures::stream::{self, StreamExt};

// Collect results in parallel
let results: Vec<_> = stream::iter(workspaces.iter())
    .map(|ws| async {
        let wt_manager = WorktreeManager::load(&ws.path).ok();
        // ... collect status for each worktree
        (ws, wt_manager, statuses)
    })
    .buffer_unordered(4)
    .collect()
    .await;

// Print results in order
for (ws, wt_manager, statuses) in &results { ... }
```

**影响范围**: `wtp-cli/src/cli/ls.rs:40-118` — 重写 long 模式逻辑。

---

### P4. 逐次写入导致 O(n^2) TOML 重写

**当前代码位置**: `wtp-core/src/worktree.rs:222-257`

```rust
// worktree.rs:251-258 (WorktreeManager::remove_worktree)
pub fn remove_worktree(&mut self, slug: &str) -> crate::Result<bool> {
    let removed = self.config.remove_by_slug(slug)
        .map_err(crate::error::WtpError::config)?;
    if removed {
        self.save()?;  // ← 每删一个就 save 一次
    }
    Ok(removed)
}
```

调用方 `remove.rs:130-132`:
```rust
for entry in &worktrees {
    worktree_manager.remove_worktree(&entry.repo.slug())?;  // N saves
}
```

**修复方案**: 新增 `remove_many` 批量 API：

```rust
// worktree.rs, WorktreeManager impl:
/// Remove multiple worktree entries by slug and save once.
pub fn remove_many(&mut self, slugs: &[&str]) -> crate::Result<usize> {
    let mut removed = 0;
    for slug in slugs {
        if self.config.remove_by_slug(slug)
            .map_err(crate::error::WtpError::config)?
        {
            removed += 1;
        }
    }
    if removed > 0 {
        self.save()?;
    }
    Ok(removed)
}
```

修改 `remove.rs:129-132`:
```rust
let mut worktree_manager = WorktreeManager::load(&workspace_path)?;
let slugs: Vec<&str> = worktrees.iter().map(|e| e.repo.slug().as_str()).collect();
// 这里有生命周期问题，改为：
let slug_strings: Vec<String> = worktrees.iter().map(|e| e.repo.slug()).collect();
let slugs: Vec<&str> = slug_strings.iter().map(|s| s.as_str()).collect();
worktree_manager.remove_many(&slugs)?;
```

**影响范围**:
- `wtp-core/src/worktree.rs` — 新增 `remove_many`
- `wtp-cli/src/cli/remove.rs:129-132` — 改用批量 API

---

## 六、安全低危 (L1, L2)

### L1. Fence 缺失时 fail-open

**当前代码位置**: `wtp-core/src/worktree.rs:151-156,222-225`

```rust
// worktree.rs:153-155 (WorktreeToml::save)
crate::fence::global_fence()
    .map(|f| f.write(path, &content))
    .unwrap_or_else(|| std::fs::write(path, content).map_err(|e| e.into()))?;

// worktree.rs:223-225 (WorktreeManager::save)
crate::fence::global_fence()
    .map(|f| self.config.save_with_fence(&self.config_path, f))
    .unwrap_or_else(|| self.config.save(&self.config_path))
```

**问题**: fence 未初始化时降级为无防护写入。

**修复方案**: fence 缺失时返回错误：

```rust
// worktree.rs:151-156, replace:
pub fn save(&self, path: &std::path::Path) -> crate::Result<()> {
    let content = toml::to_string_pretty(self)?;
    match crate::fence::global_fence() {
        Some(f) => f.write(path, &content)?,
        None => {
            // L1 fix: fail-closed when fence is not initialized
            return Err(crate::error::WtpError::config(
                "Security fence not initialized. Cannot write without boundary protection."
            ));
        }
    }
    Ok(())
}
```

同样修改 `WorktreeManager::save` (line 222-225)。

**注意**: 需确保在所有 CLI 入口点都初始化 fence（当前在 `mod.rs:370`）。对测试代码，需要在测试 setup 中初始化 fence。

**影响范围**:
- `wtp-core/src/worktree.rs:151-156,222-225` — 两处改为 fail-closed

---

### L2. Config 保存跟随 symlink

**当前代码位置**: `wtp-core/src/config.rs:83-104`

```rust
// config.rs:101
std::fs::write(&config_path, content)?;
```

**问题**: 若 `~/.wtp/config.toml` 是 symlink，则覆写目标文件。

**修复方案**: 保存前检查 symlink + 使用 temp file + atomic rename：

```rust
pub fn save(&self) -> Result<()> {
    let config_path = match &self.source_path {
        Some(path) => path.clone(),
        None => { /* ... default path ... */ }
    };

    // L2 fix: refuse to write through symlinks
    if config_path.exists() {
        let meta = std::fs::symlink_metadata(&config_path)?;
        if meta.is_symlink() {
            return Err(WtpError::config(format!(
                "Refusing to write config through symlink: {}",
                config_path.display()
            )));
        }
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(&self.config)?;

    // Atomic write: temp file + rename
    let dir = config_path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    std::io::Write::write_all(&mut tmp, content.as_bytes())?;
    tmp.persist(&config_path).map_err(|e| {
        WtpError::config(format!("Failed to persist config: {}", e))
    })?;

    Ok(())
}
```

**影响范围**:
- `wtp-core/src/config.rs:83-104` — 重写 save 方法
- `wtp-core/Cargo.toml` — 添加 `tempfile` 到 `[dependencies]`（当前仅在 dev-dependencies）

---

## 七、性能 Medium (P5-P7)

### P5. scan_git_repos 排序遍历 + 线性前缀过滤

**当前代码位置**: `wtp-core/src/git.rs:471-526`

```rust
// git.rs:480
.sort_by_file_name();

// git.rs:508
if skip_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
```

**修复方案**:

1. 删除 `.sort_by_file_name()`（调用方自行排序）。
2. 用 `WalkDir` 的 `filter_entry` 替代 `skip_prefixes` 线性查找。由于 `filter_entry` 在遍历前执行，可以利用它来剪枝：

```rust
pub fn scan_git_repos(root: &Path) -> Vec<String> {
    use walkdir::WalkDir;
    let mut repos = Vec::new();
    // Use HashSet for O(1) skip checks instead of Vec linear scan
    let mut skip_prefixes: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    let walker = WalkDir::new(root)
        .min_depth(1)
        .max_depth(4)
        .follow_links(false);
    // Remove .sort_by_file_name()

    // ... rest stays similar but use HashSet
}
```

**影响范围**: `wtp-core/src/git.rs:471-526`

---

### P6. 每次 fence 操作重复 canonicalize

**当前代码位置**: `wtp-core/src/fence.rs:55-59`

```rust
// fence.rs:56-58
let canonical_boundary = match self.boundary.canonicalize() {
    Ok(path) => path,
    Err(_) => self.boundary.clone(),
};
```

**修复方案**: 在 `Fence::new` 时缓存 canonical boundary：

```rust
pub struct Fence {
    boundary: PathBuf,
    canonical_boundary: PathBuf,  // cached
    confirm: Option<Box<dyn FenceConfirm>>,
}

impl Fence {
    pub fn new(boundary: PathBuf) -> Self {
        let canonical_boundary = boundary.canonicalize()
            .unwrap_or_else(|_| boundary.clone());
        Self {
            boundary,
            canonical_boundary,
            confirm: None,
        }
    }
}
```

然后 `is_within_boundary` 直接使用 `self.canonical_boundary`。

**影响范围**: `wtp-core/src/fence.rs` — 修改 struct + `new` + `is_within_boundary`

---

### P7. detect_current_workspace 冗余扫描

**当前代码位置**: `wtp-core/src/workspace.rs:211-229`

```rust
// workspace.rs:218
for (name, path) in self.loaded_config.scan_workspaces().iter() {
```

**问题**: 找到 `.wtp` 后仍调用 `scan_workspaces()` 扫描全部 workspace_root。

**修复方案**: 先用目录名快速检查，仅在不匹配时才全扫：

```rust
pub fn detect_current_workspace(&self) -> Result<(String, PathBuf)> {
    let current_dir = std::env::current_dir()?;
    let mut check_dir = current_dir.as_path();

    loop {
        if check_dir.join(WTP_DIR).is_dir() {
            // Quick path: if under workspace_root, directory name IS the workspace name
            let ws_root = &self.loaded_config.config.workspace_root;
            if let Ok(rel) = check_dir.strip_prefix(ws_root) {
                if rel.components().count() == 1 {
                    let name = rel.to_string_lossy().to_string();
                    return Ok((name, check_dir.to_path_buf()));
                }
            }

            // Fallback: use dir name directly (no scan needed)
            let name = check_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string();
            return Ok((name, check_dir.to_path_buf()));
        }

        match check_dir.parent() {
            Some(parent) => check_dir = parent,
            None => break,
        }
    }

    Err(WtpError::NotInWorkspace)
}
```

**影响范围**: `wtp-core/src/workspace.rs:211-239` — 重写方法。

---

## 八、性能 Low (P8)

### P8. slug/display 查找时临时分配

**当前代码位置**: `wtp-core/src/worktree.rs:71-80,177-203`

```rust
// worktree.rs:71-80 (slug())
pub fn slug(&self) -> String {
    let path = match self {
        RepoRef::Hosted { path, .. } => PathBuf::from(path),
        RepoRef::Absolute { path } => path.clone(),
    };
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}
```

**修复方案**: 添加借用版本 `slug_ref`：

```rust
/// Get slug as borrowed &str without allocation
pub fn slug_ref(&self) -> &str {
    match self {
        RepoRef::Hosted { path, .. } => {
            path.rsplit('/').next().unwrap_or(path.as_str())
        }
        RepoRef::Absolute { path } => {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        }
    }
}
```

然后在 `find_by_slug` 和 `remove_by_slug` 中使用 `slug_ref()` 替代 `slug()`。

**影响范围**: `wtp-core/src/worktree.rs` — 新增 `slug_ref`，修改 `find_by_slug`/`remove_by_slug`。

---

## 九、代码警告 (W1-W19)

### W1. config.rs:67 unwrap()

**位置**: `wtp-core/src/config.rs:67`
```rust
loaded_path.as_ref().unwrap().display()
```

**修复**: 改为 `loaded_path.as_ref().expect("loaded_path must be Some when warning is generated").display()`。或重构逻辑确保 warning 生成时 loaded_path 必定 Some。

---

### W2. detect_current_workspace 内部读 current_dir()

**位置**: `wtp-core/src/workspace.rs:212`

**修复**: 将 `current_dir` 改为参数注入：

```rust
pub fn detect_current_workspace(&self) -> Result<(String, PathBuf)> {
    self.detect_workspace_from(&std::env::current_dir()?)
}

pub fn detect_workspace_from(&self, start: &Path) -> Result<(String, PathBuf)> {
    // ... existing logic using `start` instead of `current_dir()`
}
```

**影响范围**: `wtp-core/src/workspace.rs:211-239`

---

### W3. fence.rs 全局 OnceLock 静默失败

**位置**: `wtp-core/src/fence.rs:197-198`
```rust
pub fn init_global_fence(boundary: PathBuf) {
    let _ = GLOBAL_FENCE.set(Fence::new(boundary));
}
```

**修复**: 返回 Result：
```rust
pub fn init_global_fence(boundary: PathBuf) -> std::result::Result<(), PathBuf> {
    GLOBAL_FENCE.set(Fence::new(boundary))
        .map_err(|f| f.boundary().to_path_buf())
}
```

CLI 调用方检查返回值。

---

### W4. get_ahead_behind 失败返回 (0,0)

**位置**: `wtp-core/src/git.rs:386-389`
```rust
if !output.status.success() {
    return Ok((0, 0));
}
```

**修复**: 返回 `Option<(u32,u32)>` 或 `Result` 让调用方区分：
```rust
pub fn get_ahead_behind(&self, repo_path: &Path, base: &str) -> Result<Option<(u32, u32)>> {
    // ...
    if !output.status.success() {
        return Ok(None);  // base ref doesn't exist
    }
    // ...
    Ok(Some((ahead, behind)))
}
```

**影响范围**: `wtp-core/src/git.rs:377-400` + 所有调用方（status.rs, ls.rs）

---

### W5. get_status 手动解析 porcelain

**本轮不修改** — P1 新增的 `get_full_status` (porcelain=v2) 将逐步取代。

---

### W6. count() as u32 截断

**位置**: `wtp-core/src/git.rs:417-420`
```rust
.count() as u32;
```

**修复**:
```rust
let count: u32 = String::from_utf8_lossy(&output.stdout)
    .lines()
    .filter(|l| !l.is_empty())
    .count()
    .try_into()
    .unwrap_or(u32::MAX);
```

---

### W7. remove_by_slug 返回 Result<bool, String>

**位置**: `wtp-core/src/worktree.rs:185`
```rust
pub fn remove_by_slug(&mut self, slug: &str) -> std::result::Result<bool, String> {
```

**修复**: 改为 `crate::Result<bool>`：
```rust
pub fn remove_by_slug(&mut self, slug: &str) -> crate::Result<bool> {
    // ...
    if matches.len() > 1 {
        return Err(WtpError::config(format!(...)));
    }
    // ...
}
```

调用方 `WorktreeManager::remove_worktree` 不再需要 `.map_err`。

---

### W8. config.rs emoji 警告字符串

**位置**: `wtp-core/src/config.rs:64-65`
```rust
Some(format!(
    "⚠️  Warning: Multiple config files found: {}. Using {}",
```

**修复**: 去除 emoji，改为纯文本：
```rust
Some(format!(
    "Warning: Multiple config files found: {}. Using {}",
```

由 CLI 层添加 emoji（如需要）。

---

### W9. hosts 用 HashMap 但 Cargo.toml 有 indexmap

**位置**: `wtp-core/src/config.rs:125`

**修复**: 改为 `IndexMap` 以保持配置文件中的插入顺序：
```rust
use indexmap::IndexMap;
pub hosts: IndexMap<String, HostConfig>,
```

---

### W10. 核心库引入 tokio rt-multi-thread

**位置**: `wtp-core/Cargo.toml:24`

**修复**: 最小化 tokio features — core 只需 `process`：
```toml
tokio = { version = "1.40", features = ["process"] }
```

但注意 `workspace.rs:112` 使用 `tokio::process::Command`，确认只需 `process` feature。`rt-multi-thread` 由 CLI 引入。

---

### W11. import.rs 死代码

**位置**: `wtp-cli/src/cli/import.rs:75-77`
```rust
if args.path.is_some() {
    anyhow::bail!("Cannot specify both <path> argument and --repo");
}
```

**修复**: 删除这 3 行（clap `conflicts_with` 已处理）。

---

### W12. mod.rs loaded_config move 模式

**本轮不修改** — 属于代码风格，重构 risk 高。

---

### W13. import/switch 重复 worktree 创建逻辑

**位置**: `import.rs:160-189` 和 `switch.rs:200-242` 约 140 行近乎相同。

**修复**: 提取共享函数到 `wtp-cli/src/cli/common.rs`：

```rust
pub fn create_worktree_in_workspace(
    git: &GitClient,
    repo_root: &Path,
    workspace_path: &Path,
    repo_ref: &RepoRef,
    branch: &str,
    base: &str,
    worktree_manager: &WorktreeManager,
) -> anyhow::Result<(PathBuf, WorktreeEntry)> {
    // shared logic: check existing, generate path, create worktree, get HEAD, build entry
}
```

**影响范围**:
- 新增 `wtp-cli/src/cli/common.rs`
- `wtp-cli/src/cli/import.rs` — 调用共享函数
- `wtp-cli/src/cli/switch.rs` — 调用共享函数
- `wtp-cli/src/cli/mod.rs` — 添加 `pub mod common;`

---

### W14. import/switch 重复 fence 边界检查

**位置**: `import.rs:52-70` 和 `switch.rs:121-142`

**修复**: 纳入 W13 的共享模块：

```rust
pub fn check_workspace_boundary(
    fence: &Fence,
    workspace_name: &str,
    workspace_path: &Path,
) -> anyhow::Result<()> {
    // shared fence check + interactive confirm
}
```

---

### W15. eject.rs 重复 skim 选择

**位置**: `eject.rs:116-211`

**本轮不修改** — `fuzzy.rs` 的 `select_from_list` 未完全匹配 eject 的需求（需要 key+display 双值），重构 risk 中等。

---

### W16. git.get_status() 错误静默吞掉

**位置**: `ls.rs:80-83`, `status.rs:84-87`, `eject.rs`/`remove.rs` 多处

```rust
Ok(s) => s.format_compact(),
Err(_) => "?".to_string(),
```

**修复**: 统一为 `Err(e) => format!("! {}", e).red().to_string()` 或至少 `tracing::debug!`。

---

### W17. HEAD 信息分三次 git 命令

**已在 P1 中修复** — `get_head_info` 合并查询。

---

### W18. 自定义 help 系统 260+ 行

**本轮不修改** — 功能正常，重构 risk 高。

---

### W19. 手写补全脚本 290 行

**本轮不修改** — 转用 clap_complete 是大改动。

---

## 十、代码建议 (A1-A19)

### A1. GitClient 无法 mock

**本轮不修改** — 引入 trait 需要大范围改动。后续可考虑。

---

### A2. git 命令方法模式重复

**修复**: 在 P1 合并查询时顺便提取 `run_git` helper：

```rust
fn run_git(&self, repo_path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(WtpError::git(
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

**影响范围**: `wtp-core/src/git.rs` — 新增 helper，逐步重构现有方法。

---

### A3. WorktreeToml::Default 产生 version: ""

**位置**: `wtp-core/src/worktree.rs:124`

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorktreeToml {
    pub version: String,  // Default → ""
```

**修复**: 自定义 Default：

```rust
impl Default for WorktreeToml {
    fn default() -> Self {
        Self::new()  // version: "1"
    }
}
```

移除 derive 中的 `Default`，手动实现。

---

### A4. WorktreeToml 未 re-export

**位置**: `wtp-core/src/lib.rs:17`

**修复**: 添加 re-export：
```rust
pub use worktree::{RepoRef, WorktreeEntry, WorktreeManager, WorktreeToml};
```

---

### A5. error.rs 缺少 Hook 变体

**位置**: `wtp-core/src/error.rs`

**修复**: 新增：
```rust
#[error("Hook error: {0}")]
Hook(String),

#[error("UUID error: {0}")]
Uuid(#[from] uuid::Error),
```

---

### A6. create_workspace async 但多数是同步 I/O

**本轮不修改** — 仅 hook 调用是真正 async，其余是同步但量小。

---

### A7. is_within_boundary 存在/不存在路径走不同逻辑

**已在 H1/P6 中改进** — 缓存 canonical_boundary + symlink 检查。

---

### A8. tilde 展开 to_string_lossy

**位置**: `wtp-core/src/config.rs:41`

**修复**: 添加注释说明限制，或改用 OsString 版本：

```rust
// Note: shellexpand only works with UTF-8 paths. Non-UTF-8 paths
// are lossy-converted, which may cause issues on some systems.
```

**本轮仅添加注释**，不改功能。

---

### A9. list_workspaces 返回无序

**位置**: `wtp-core/src/workspace.rs:36-48` + `config.rs:206-226`

**修复**: `scan_workspaces` 返回 `BTreeMap` 替代 `HashMap`，或在 W9 使用 `IndexMap` 后排序输出。

简单方案 — `list_workspaces` 排序：
```rust
pub fn list_workspaces(&self) -> Vec<WorkspaceInfo> {
    let mut workspaces: Vec<_> = self.loaded_config
        .scan_workspaces()
        .into_iter()
        .map(|(name, path)| WorkspaceInfo { name, path, exists: true })
        .collect();
    workspaces.sort_by(|a, b| a.name.cmp(&b.name));
    workspaces
}
```

---

### A10. scan_git_repos O(n*m) skip-prefix

**已在 P5 中修复** — 改用 HashSet。

---

### A11. 重复 use colored::Colorize

**位置**: `wtp-cli/src/cli/mod.rs:33`（在函数内部重复 import）

**修复**: 删除 `mod.rs:33` 的 `use colored::Colorize;`（已在文件顶部 line 20 通过 Cli 的 derive 间接导入，但实际上 line 33 是函数内部的）。检查后发现 line 20 没有此 import，line 33 在 `print_styled_help` 内部 — 保持不变（函数内部 import 虽冗余但无害）。

**本轮不修改。**

---

### A12. wtp-cli tokio rt-multi-thread 可能过度

**保留** — CLI 需要 `#[tokio::main]` 运行时，`rt-multi-thread` 是必需的。

---

### A13. args 读取两次

**位置**: `wtp-cli/src/cli/mod.rs:344`
```rust
let args: Vec<String> = std::env::args().collect();
```

后在 line 352 `Cli::parse()` 再次读取 args。

**修复**: 使用 `Cli::try_parse_from(&args)`：
```rust
let args: Vec<String> = std::env::args().collect();
let cmd = Cli::command().styles(theme::wtp_styles());
if try_show_help(&cmd, &args) {
    return Ok(());
}
let cli = Cli::try_parse_from(&args)?;
```

---

### A14. ls.rs 截断逻辑

**位置**: `wtp-cli/src/cli/status.rs:91-96`（实际在 status.rs 而非 ls.rs）

```rust
if repo_display.chars().count() > 30 {
    let truncated: String = repo_display.chars().take(27).collect();
    format!("{}...", truncated)
}
```

**修复**: 用 `char_indices` 优化为单次遍历：
```rust
fn truncate_display(s: &str, max_len: usize) -> String {
    let mut end = 0;
    for (i, (idx, _)) in s.char_indices().enumerate() {
        if i >= max_len - 3 {
            return format!("{}...", &s[..idx]);
        }
        end = idx;
    }
    s.to_string()
}
```

**影响范围**: 可放入 `git_status_fmt.rs` 或新增 `util.rs`。

---

### A15. async fn 内无 .await

**位置**: `wtp-cli/src/cli/status.rs:58,105`

**修复**: 当 P2 提供 async git 方法后自然解决。当前保持 async 签名以便未来使用。

---

### A16. host.rs async 无 await

**与 A15 同理，保持现状。**

---

### A17. 冗余 format!

**位置**: `wtp-cli/src/cli/git_status_fmt.rs:40-41` 等

```rust
format!("{}", format!("+{}", self.ahead).green())
```

**修复**: 简化为：
```rust
format!("+{}", self.ahead).green().to_string()
```

**影响范围**: `git_status_fmt.rs` 多处。

---

### A18. cd.rs 路径转义

**本轮不修改** — 添加文档注释即可。

---

### A19. 无 fish shell-init 支持

**本轮不修改** — completions 已生成 fish 补全，shell-init 的 fish 版本需要新逻辑。

---

## 十一、修复优先级与分组

### Batch 1: 安全高危（必须先修）
- **H1** — fence symlink + TOCTOU（fence.rs）
- **H2** — eject 路径遍历（fence.rs + eject.rs）
- **H3** — remove 路径遍历（remove.rs）
- **L1** — fence fail-open（worktree.rs）

### Batch 2: 安全中危
- **M1** — hook 相对路径（config.rs）
- **M2** — git 参数注入（git.rs）
- **M3** — import 路径遍历（import.rs）
- **L2** — config symlink（config.rs）

### Batch 3: 核心代码质量
- **S1** — core 库 stdin/stderr（fence.rs）
- **S2** — 库 println（workspace.rs）
- **S3** — RepoRef 类型签名（worktree.rs）
- **S5** — 分支回退警告（switch.rs, import.rs）
- **S6** — macOS grep -oP（completions.rs）

### Batch 4: 性能
- **P1** — status 合并 git 命令（git.rs, status.rs）
- **P4** — 批量 TOML（worktree.rs, remove.rs）
- **P6** — fence canonicalize 缓存（fence.rs）
- **P7** — detect_current_workspace（workspace.rs）

### Batch 5: 警告修复
- **W1** — unwrap 改 expect
- **W2** — detect_current_workspace 参数注入
- **W3** — OnceLock 返回 Result
- **W4** — get_ahead_behind 返回 Option
- **W6** — count as u32 截断
- **W7** — remove_by_slug 错误类型
- **W8** — emoji 警告
- **W9** — IndexMap
- **W10** — tokio features 最小化
- **W11** — 死代码
- **W13/W14** — import/switch 重复消除
- **W16** — 错误静默

### Batch 6: 建议
- **A2** — run_git helper
- **A3** — Default version
- **A4** — re-export
- **A5** — error 变体
- **A9** — 排序
- **A13** — args 读取两次
- **A14** — 截断优化
- **A17** — 冗余 format

---

*修复设计文档完成*
