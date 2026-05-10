# wtp Workspace 重构 — 统一审计报告

**审计日期**: 2026-03-28
**审计范围**: wtp-core (7 files), wtp-cli (17 files), wtp-gui (13 files) — 共 37 个 Rust 源文件
**审计方式**: security-engineer 和 perf-benchmarker 通过 Codex MCP (GPT-5.4, xhigh); code-reviewer 手动完成

---

## 总览

| 维度 | 严重/高危 | 警告/中危 | 建议/低危 | 合计 |
|------|----------|----------|----------|------|
| 代码质量 | 9 | 24 | 23 | 56 |
| 安全 | 3 | 4 | 2 | 9 |
| 性能 | 1 | 3 | 4 | 8 |
| **合计** | **13** | **31** | **29** | **73** |

### 基础检查

| 检查项 | 结果 |
|--------|------|
| 依赖隔离 (wtp-core 无 CLI 依赖) | ✅ PASS |
| 路径替换 (零 `crate::core::` 残留) | ✅ PASS |
| GitStatusFormat trait | ✅ 正确实现 |
| `cargo check --workspace` | ✅ 通过 |
| `cargo test --workspace` | ✅ 20/20 通过 |

---

## 一、安全审计

> 审计员: security-engineer | 工具: Codex GPT-5.4 (read-only, xhigh)

### 高危 (3)

**H1. [HIGH] wtp-core/src/fence.rs:61,126 — Symlink TOCTOU 可绕过 fence 边界 | CWE-59**
- **问题**: `Fence::create_dir_all` 依赖词法规范化检查路径是否在 workspace_root 内，但对不存在的路径只在创建后才验证。如果中间目录是 symlink（如 `workspace_root/link → /tmp/outside`），`std::fs::create_dir_all` 会跟随 symlink 在边界外创建目录。
- **利用**: 本地攻击者在 workspace_root 下放置 symlink，可让 `create_workspace()` 在 workspace 外创建任意目录。
- **修复**: 在创建前检查所有祖先路径是否含 symlink，或使用 `openat`/`mkdirat` + `O_NOFOLLOW` 语义逐级创建。

**H2. [HIGH] wtp-cli/src/cli/eject.rs:46-108 — 恶意 worktree 元数据可操作外部 worktree | CWE-22**
- **问题**: `wtp eject` 从 `.wtp/worktree.toml` 读取 `worktree_path`，直接拼接到 workspace_path 上执行 git status 和 `git worktree remove`，无规范化边界检查。
- **利用**: 篡改 `.wtp/worktree.toml` 设置 `worktree_path = "../../victim"` 可操作 workspace 外的 worktree。
- **修复**: 规范化 `workspace_path.join(worktree_path)` 后验证是否仍在 workspace 目录内，否则拒绝操作。

**H3. [HIGH] wtp-cli/src/cli/remove.rs:35-160 — 批量删除可操作外部 worktree | CWE-22**
- **问题**: 与 H2 同源，`wtp remove` 加载所有 worktree 路径后逐个执行 git worktree remove，均无边界校验。
- **利用**: 单条 `wtp remove <name>` 可批量删除 workspace 外的 worktree。
- **修复**: 所有 entry 都需规范化 + 边界检查，任何 entry 越界则中止整个删除。

### 中危 (4)

**M1. [MEDIUM] wtp-core/src/config.rs:50 + workspace.rs:85 — 相对路径 hook 执行 | CWE-426**
- **问题**: `hooks.on_create` 配置为相对路径时，从调用者的 CWD 解析，可能执行恶意仓库中的脚本。
- **修复**: 要求 hook 路径必须为绝对路径，或相对于配置文件所在目录解析。

**M2. [MEDIUM] wtp-core/src/git.rs:165,205,352 — Git 参数注入 | CWE-88**
- **问题**: 多个 git 封装函数传递用户输入作为位置参数时未加 `--` 分隔符。虽然使用 `Command::new().arg()` 避免了 shell 注入，但 Git 仍会将 `-` 开头的值当选项解析。
- **修复**: 在用户可控的位置参数前插入 `--`，并拒绝 `-` 开头的 branch/base/path 值。

**M3. [MEDIUM] wtp-cli/src/cli/import.rs:84-264 — Host 路径遍历 + Git 参数注入 | CWE-22, CWE-88**
- **问题**: hosted 模式下 repo 路径直接 join 到 host_root，`../../other/repo` 可逃逸命名空间。branch/base 同样存在参数注入。
- **修复**: 规范化后验证在 host_root 内；branch/base 用 `git check-ref-format` 校验。

**M4. [MEDIUM] wtp-cli/src/cli/switch.rs:165-227 — Git 参数注入 | CWE-88**
- **问题**: 与 M2/M3 同源，switch 命令的 branch/base 参数同样未经校验直传 git。
- **修复**: 抽取共享校验辅助函数，统一处理 import 和 switch 的 ref 校验。

### 低危 (2)

**L1. [LOW] wtp-core/src/worktree.rs:151,222 — Fence 缺失时 fail-open | CWE-693**
- **问题**: `WorktreeToml::save()` 和 `WorktreeManager::save()` 在全局 fence 未初始化时静默降级为无防护写入。
- **修复**: fence 未设置时返回错误，而非降级写入。

**L2. [LOW] wtp-cli/src/cli/host.rs:91-206 — Config 保存跟随 symlink | CWE-59**
- **问题**: config 保存通过 `std::fs::write` 直写，若 `~/.wtp/config.toml` 是 symlink 则会覆写目标文件。
- **修复**: 拒绝 symlink 配置文件路径；使用 temp file + atomic rename 写入。

### 安全审计覆盖总结

| 维度 | 结果 |
|------|------|
| Fence 机制完整性 | H1, L1 — symlink 绕过 + fail-open |
| 命令注入 | M2, M3, M4 — Git 参数注入（非 shell 注入） |
| 路径遍历 | H2, H3, M3 — 元数据路径信任 + host 路径逃逸 |
| Hook 脚本执行 | M1 — 相对路径 hook 执行风险 |
| 敏感数据 | 无发现 |
| 依赖安全 | 无已知漏洞 |
| GUI 安全 | 无发现（stub 状态） |
| TOCTOU 竞争 | H1 — fence create_dir_all |
| Unsafe 代码 | 无发现 |

---

## 二、性能审计

> 审计员: perf-benchmarker | 工具: Codex GPT-5.4 (read-only, xhigh)

### CRITICAL (1)

**P1. [性能] wtp-cli/src/cli/status.rs:105-223 — `wtp status --long` 每仓库 5-6 次串行 git fork**
- **影响**: 10 个仓库 = 50-60 次串行子进程调用
- **详情**: `print_detailed_status` 依次调用 `get_ahead_behind`、`get_head_commit`、`get_last_commit_subject`、`get_last_commit_relative_time`、`get_status`、`get_stash_count`，无批处理、无并行
- **修复**: 合并为 2 条 git 命令：`git status --porcelain=v2 --branch --show-stash` + `git log -1 --format=%h%x00%s%x00%cr`；用 `buffer_unordered(4..8)` 跨仓库并行

### HIGH (3)

**P2. [I/O] wtp-core/src/git.rs:20-421 — async 上下文中全同步 GitClient**
- **详情**: 所有 GitClient 方法使用 `std::process::Command::output()`，在 `#[tokio::main]` 下阻塞 tokio worker。`get_repo_root` 最多 fork 3 次 git 进程做一次逻辑查询。
- **修复**: 切换到 `tokio::process::Command` 或 `spawn_blocking`；`rev-parse` 可合并为一次调用

**P3. [并发] wtp-cli/src/cli/ls.rs:40-118 — `wtp ls --long` 跨所有 workspace 串行探测**
- **影响**: 10 个 workspace × 5 个 repo = 50-100 次串行 git fork
- **修复**: 构建 `collect_repo_summary` 辅助函数，用有界并行执行，先收集结果再按序打印

**P4. [序列化] wtp-core/src/worktree.rs:222-257 — 逐次写入导致批量操作 O(n²) TOML 重写**
- **影响**: 删除 50 个 worktree 会重写 50 次 `worktree.toml`，序列化 1,225 个总条目
- **修复**: 提供 `remove_many` / `config_mut() + save_once()` 批量 API

### MEDIUM (3)

**P5. [内存] wtp-core/src/git.rs:471-526 — `scan_git_repos` 排序遍历 + 线性前缀过滤**
- **影响**: 10,000 目录 + 500 仓库 ≈ 5,000,000 次 `starts_with` 检查
- **详情**: `sort_by_file_name()` 强制排序不一定必要
- **修复**: 改用 `WalkDir::skip_current_dir()`，去掉排序，优化 String 分配

**P6. [I/O] wtp-core/src/fence.rs:55-181 — 每次文件操作重复 canonicalize 边界路径**
- **影响**: 每次 fence 操作 2-4 次多余 `canonicalize` 系统调用
- **修复**: 创建 Fence 时缓存 canonical boundary

**P7. [性能] wtp-core/src/workspace.rs:211-229 — `detect_current_workspace` 冗余扫描 workspace_root**
- **影响**: 找到 `.wtp` 后仍调用 `scan_workspaces()` 仅为获取 workspace 名称。500 个 workspace = 每次命令多扫描 500 个目录项。
- **修复**: 直接返回 `check_dir.file_name()` 作为名称

### LOW (1)

**P8. [内存] wtp-core/src/worktree.rs:171-202 — slug/display 查找时线性扫描中临时分配**
- **详情**: `find_by_slug`/`remove_by_slug` 每个条目创建临时 `PathBuf`/`String`
- **修复**: 添加借用版辅助方法 `slug_str(&self) -> Option<&str>`

### 性能审计总结

| 严重程度 | 数量 | 主要瓶颈 |
|---------|------|---------|
| Critical | 1 | 串行 git fork 累积延迟 |
| High | 3 | 阻塞 I/O + 无并行 + O(n²) 序列化 |
| Medium | 3 | 冗余系统调用 + 内存分配 |
| Low | 1 | 临时对象分配 |

**核心结论**: 最大性能瓶颈在于 `status --long` 和 `ls --long` 的串行 git 进程 fork。优化合并 git 命令 + 并行执行可预期 **5-10x 延迟改善**。

---

## 三、代码质量审计

> 审计员: code-reviewer | 工具: 手动（MCP 工具不可用）

### 严重 (9)

**S1. [严重] wtp-core/src/fence.rs:102-107 — Core 库直接读写 stdin/stderr**
- 核心库 `fence.rs` 中直接 `eprintln!` + `stdin().read_line()` 进行交互式确认。这使 wtp-core 耦合终端环境，GUI 和测试无法使用。
- **修复**: 改为回调 trait 或返回结构化结果让调用方处理 I/O。

**S2. [严重] wtp-core/src/workspace.rs:77,125 — 库代码中 println!/eprintln!**
- `create_workspace` 中直接 `eprintln!("Warning: Failed to run create hook: {}")` 和 `println!("{}", stdout)`。
- **修复**: 库应返回结构化数据，由 CLI/GUI 决定展示方式。

**S3. [严重] wtp-core/src/worktree.rs:48-59 — `RepoRef::to_absolute_path` 类型签名不匹配**
- 接受 `HashMap<String, PathBuf>` 但配置存储的是 `HashMap<String, HostConfig>`。每个调用方必须手动提取 `.root`。fallback `PathBuf::from(path)` 在找不到 host 时静默产生错误的相对路径而非报错。

**S4. [严重] wtp-core/src/fence.rs:126-133 — TOCTOU 竞态：create_dir_all 后 verify_canonical**
- `create_dir_all` 和 `verify_canonical` 之间存在窗口期，攻击者可替换目录为符号链接。（与安全 H1 同源）

**S5. [严重] wtp-cli/src/cli/switch.rs:166-173 & import.rs:120-125 — 分支基准回退静默丢失上下文**
- `get_current_branch` 失败时静默回退到 `"HEAD"`。对 bare repo 的 HEAD 解析可能出人意料。应至少 `tracing::warn!` 记录回退。

**S6. [严重] wtp-cli/src/cli/completions.rs:164,241 — `grep -oP` 在 macOS 不可用**
- zsh 和 bash 补全脚本使用 `grep -oP`（Perl regex），macOS 默认 grep 不支持。直接导致 macOS 用户补全失败。应改用 `awk`/`sed`。

**S7. [严重] wtp-gui/Cargo.toml:20 — GPUI 依赖完全注释掉，crate 不可用**
- GUI 框架依赖是 `TBD_PIN_COMMIT_HASH` 占位。但 workspace 编译仍拉取 `tray-icon`/`tokio`/`smol`。
- **修复**: 从 workspace 默认构建中排除 wtp-gui。

**S8. [严重] wtp-gui/src/state.rs:30-51 — AppState::load() 阻塞 I/O 在 GUI 线程**
- `LoadedConfig::load()` 做同步文件 I/O，计划在 GPUI 主线程调用。会冻结窗口。
- **修复**: 改为 async 或后台任务。

**S9. [严重] wtp-gui/src/state.rs:31-32 — 配置加载警告被静默丢弃**
- `LoadedConfig::load()` 返回的 `_warning` 被直接忽略。

### 警告 (24)

#### wtp-core (10)

| # | 文件:行 | 描述 |
|---|---------|------|
| W1 | config.rs:67 | `unwrap()` 在非测试代码中。`loaded_path` 的 `Some` 不变量隐式依赖上下文 |
| W2 | workspace.rs:212 | `detect_current_workspace` 内部读 `current_dir()`，不可测试 |
| W3 | fence.rs:192-215 | 全局 `OnceLock` 单例：第二次 `init_global_fence` 静默失败 |
| W4 | git.rs:386-389 | `get_ahead_behind` 失败时返回 `(0,0)` 而非 `None`，无法区分"同步"与"分支不存在" |
| W5 | git.rs:236-305 | `get_status` 70 行手动解析 `--porcelain --branch`，`ahead`/`behind` 用硬编码偏移 |
| W6 | git.rs:417-420 | `count() as u32` 在 64 位系统静默截断 |
| W7 | worktree.rs:185-203 | `remove_by_slug` 返回 `Result<bool, String>` 而非 `WtpError`，破坏错误处理一致性 |
| W8 | config.rs:65 | 核心库输出含 emoji 的警告字符串，混入展示层逻辑 |
| W9 | config.rs:125 | `hosts` 用 `HashMap` 但 Cargo.toml 声明了 `indexmap`（未使用或应该用） |
| W10 | Cargo.toml:24 | 核心库引入 `tokio rt-multi-thread`，仅为一个 `Command` 调用 |

#### wtp-cli (9)

| # | 文件:行 | 描述 |
|---|---------|------|
| W11 | import.rs:73-76 | 死代码：`path`/`--repo` 冲突检查已由 clap `conflicts_with` 处理 |
| W12 | mod.rs:343-389 | `loaded_config` 在每个 match arm 中 move，模式脆弱 |
| W13 | import.rs & switch.rs | ~140 行重复的 worktree 创建逻辑 |
| W14 | import.rs & switch.rs | ~15 行重复的 fence 边界检查逻辑 |
| W15 | eject.rs:116-211 | 重复实现 skim 选择模式，fuzzy.rs 已有 `select_from_list` |
| W16 | ls.rs/status.rs/eject.rs/remove.rs | `git.get_status()` 错误被静默吞掉，替换为 `"?"` |
| W17 | status.rs:163-169 | HEAD 信息分三次 git 命令获取，应合并为一次 |
| W18 | mod.rs:24-285 | 自定义 help 系统 260+ 行，与 clap 内置能力重复 |
| W19 | completions.rs:32-321 | 手写补全脚本 290 行，新增子命令时必须手动同步 |

#### wtp-gui (5)

| # | 文件:行 | 描述 |
|---|---------|------|
| W20 | Cargo.toml:26,32 | 同时依赖 `tokio` 和 `smol` 两个 async 运行时 |
| W21 | state.rs:42,55 | `LoadedConfig` 每次 refresh 都 clone，应借用或持久化 Manager |
| W22 | views/*.rs, components/*.rs | 7 个空 stub 模块，无编译时保证 |
| W23 | state.rs:10-11 | `ViewState` 用裸 `String` 标识 workspace，应用 newtype |
| W24 | state.rs:33-39 | 配置加载错误 `eprintln!` 到 stderr，GUI 用户看不到 |

### 建议 (23)

#### wtp-core (10)

| # | 文件 | 描述 |
|---|------|------|
| A1 | git.rs:12-17 | `GitClient` 无状态 unit struct，无法 mock 测试 |
| A2 | git.rs 全文件 | 10+ 个 git 命令方法模式重复，应提取 `run_git` helper |
| A3 | worktree.rs:124-138 | `WorktreeToml::Default` 产生 `version: ""` 而 `new()` 产生 `"1"`，语义不一致 |
| A4 | lib.rs:17 | `WorktreeToml` 未 re-export，消费者需用全路径 |
| A5 | error.rs | 缺少 `Hook` 错误变体和 `From<uuid::Error>` |
| A6 | workspace.rs:51 | `create_workspace` 是 async 但大部分是同步 I/O，阻塞运行时 |
| A7 | fence.rs:55-79 | `is_within_boundary` 对存在/不存在路径走不同逻辑，安全决策不一致 |
| A8 | config.rs:41-48 | tilde 展开用 `to_string_lossy()` 可能在 non-UTF-8 路径上损坏数据 |
| A9 | workspace.rs:36-48 | `list_workspaces` 返回无序结果（HashMap 迭代） |
| A10 | git.rs:508-509 | `scan_git_repos` O(n*m) skip-prefix 检查，可用 `filter_entry` 优化 |

#### wtp-cli (9)

| # | 文件 | 描述 |
|---|------|------|
| A11 | mod.rs:33 | 重复 `use colored::Colorize` import |
| A12 | Cargo.toml:28 | `tokio rt-multi-thread` 可能过度，当前无真正 async I/O |
| A13 | mod.rs:344 | `std::env::args().collect()` 导致 args 读取两次 |
| A14 | ls.rs:91-96 | 截断逻辑两次迭代 chars，可用 `char_indices` 单次完成 |
| A15 | status.rs:58-103,105-231 | `async fn` 内无 `.await`，不必要的 Future 包装 |
| A16 | host.rs:49-215 | 四个 host 子命令 handler 是 async 但无 await |
| A17 | git_status_fmt.rs:40-41 等 | 多处 `format!("{}", x.yellow())` 冗余，用 `.to_string()` 即可 |
| A18 | cd.rs:50 | 路径转义方案对特殊字符（`$`, backtick）的完整性值得记录 |
| A19 | shell_init.rs | 无 fish shell 支持，但 completions 生成了 fish 补全 |

#### wtp-gui (4)

| # | 文件 | 描述 |
|---|------|------|
| A20 | tray.rs, app.rs | 含注释掉的 import 行，应删除或转为设计文档 |
| A21 | state.rs | 无测试覆盖 |
| A22 | 全 crate | 模块文档术语不统一 |
| A23 | main.rs:10 | `mod components` 声明但从未使用 |

---

## 四、交叉引用：多维度重叠问题

以下问题被多个审计维度同时识别，优先级最高：

| 问题 | 安全 | 性能 | 代码质量 |
|------|------|------|---------|
| fence.rs TOCTOU/symlink | H1 | P6 | S4 |
| GitClient 全同步阻塞 | — | P2 | A6 |
| worktree 路径无边界检查 | H2, H3 | — | — |
| Git 参数注入 | M2, M3, M4 | — | — |
| core 库含交互式 I/O | — | — | S1, S2 |
| status/ls 串行 git fork | — | P1, P3 | W17 |

---

## 五、修复优先级建议

### 立即修复（安全高危）
1. H1 — fence symlink TOCTOU
2. H2 — eject 路径遍历
3. H3 — remove 路径遍历

### 本迭代修复（安全中危 + 性能关键 + 代码严重）
4. M1-M4 — hook 路径 + Git 参数注入
5. P1 — status --long 串行 git fork
6. S1, S2 — core 库交互式 I/O 提取
7. S6 — macOS grep -oP 补全失效
8. P2 — GitClient 全同步阻塞

### 下迭代排期
9. S3 — RepoRef::to_absolute_path 类型签名
10. P3, P4 — ls 并行 + 批量 TOML 写入
11. W13, W14 — import/switch 代码重复消除
12. L1, L2 — fence fail-open + config symlink
13. 其余 Warning 和建议项

---

*报告结束*
