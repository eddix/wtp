# wtp 代码质量审计报告

**审计范围**: wtp-core (7 files), wtp-cli (17 files), wtp-gui (14 files)
**审计维度**: 依赖隔离、路径替换完整性、正确性、可维护性、可读性、GitStatusFormat trait

---

## 总体评价

| Crate | 状态 | 严重 | 警告 | 建议 |
|-------|------|------|------|------|
| wtp-core | 可用，有架构问题 | 4 | 10 | 10 |
| wtp-cli | 可用，有重复代码和兼容性问题 | 2 | 9 | 9 |
| wtp-gui | 骨架状态，不可编译运行 | 3 | 5 | 4 |
| **合计** | | **9** | **24** | **23** |

**依赖隔离**: ✅ PASS — wtp-core 无 CLI/TUI 依赖（colored/clap/ratatui 等均不存在）
**路径替换**: ✅ PASS — 源码中零 `crate::core::` 残留
**GitStatusFormat trait**: ✅ 正确实现并一致使用

---

## 🔴 严重问题（9 个）

### wtp-core

**[严重] wtp-core/src/fence.rs:102-107 — Core 库直接读写 stdin/stderr**
核心库 `fence.rs` 中直接 `eprintln!` + `stdin().read_line()` 进行交互式确认。这使 wtp-core 耦合终端环境，GUI 和测试无法使用。应改为回调 trait 或返回结构化结果让调用方处理 I/O。

**[严重] wtp-core/src/workspace.rs:77,125 — 库代码中 println!/eprintln!**
`create_workspace` 中直接 `eprintln!("Warning: Failed to run create hook: {}")` 和 `println!("{}", stdout)`。库应返回结构化数据，由 CLI/GUI 决定展示方式。

**[严重] wtp-core/src/worktree.rs:48-59 — `RepoRef::to_absolute_path` 类型签名不匹配**
接受 `HashMap<String, PathBuf>` 但配置存储的是 `HashMap<String, HostConfig>`。每个调用方必须手动提取 `.root`。fallback `PathBuf::from(path)` 在找不到 host 时静默产生错误的相对路径而非报错。

**[严重] wtp-core/src/fence.rs:126-133 — TOCTOU 竞态：create_dir_all 后 verify_canonical**
`create_dir_all` 和 `verify_canonical` 之间存在窗口期，攻击者可替换目录为符号链接。代码注释也承认这是竞态。应使用 `O_NOFOLLOW` 或 `openat` 方案。

### wtp-cli

**[严重] wtp-cli/src/cli/switch.rs:166-173 & import.rs:120-125 — 分支基准回退静默丢失上下文**
`get_current_branch` 失败时静默回退到 `"HEAD"`。对 bare repo 的 HEAD 解析可能出人意料。应至少 `tracing::warn!` 记录回退。

**[严重] wtp-cli/src/cli/completions.rs:164,241 — `grep -oP` 在 macOS 不可用**
zsh 和 bash 补全脚本使用 `grep -oP`（Perl regex），macOS 默认 grep 不支持。直接导致 macOS 用户补全失败。应改用 `awk`/`sed`。

### wtp-gui

**[严重] wtp-gui/Cargo.toml:20 — GPUI 依赖完全注释掉，crate 不可用**
GUI 框架依赖是 `TBD_PIN_COMMIT_HASH` 占位。二进制只打印错误后 exit(1)。但 workspace 编译仍拉取 `tray-icon`/`tokio`/`smol`，浪费 CI 时间。应从 workspace 默认构建中排除。

**[严重] wtp-gui/src/state.rs:30-51 — AppState::load() 阻塞 I/O 在 GUI 线程**
`LoadedConfig::load()` 做同步文件 I/O，计划在 GPUI 主线程调用。会冻结窗口。应改为 async 或后台任务。

**[严重] wtp-gui/src/state.rs:31-32 — 配置加载警告被静默丢弃**
`LoadedConfig::load()` 返回的 `_warning`（多配置文件警告）被直接忽略。GUI 用户无法得知配置歧义。

---

## 🟡 警告问题（24 个）

### wtp-core（10 个）

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

### wtp-cli（9 个）

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

### wtp-gui（5 个）

| # | 文件:行 | 描述 |
|---|---------|------|
| W20 | Cargo.toml:26,32 | 同时依赖 `tokio` 和 `smol` 两个 async 运行时 |
| W21 | state.rs:42,55 | `LoadedConfig` 每次 refresh 都 clone，应借用或持久化 Manager |
| W22 | views/*.rs, components/*.rs | 7 个空 stub 模块，无编译时保证 |
| W23 | state.rs:10-11 | `ViewState` 用裸 `String` 标识 workspace，应用 newtype |
| W24 | state.rs:33-39 | 配置加载错误 `eprintln!` 到 stderr，GUI 用户看不到 |

---

## 💭 建议（23 个）

### wtp-core（10 个）

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

### wtp-cli（9 个）

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

### wtp-gui（4 个）

| # | 文件 | 描述 |
|---|------|------|
| A20 | tray.rs, app.rs | 含注释掉的 import 行，应删除或转为设计文档 |
| A21 | state.rs | 无测试覆盖 |
| A22 | 全 crate | 模块文档术语不统一 |
| A23 | main.rs:10 | `mod components` 声明但从未使用 |

---

## Top 5 推荐改进优先级

1. **🔴 从 wtp-core 中提取所有交互式 I/O**（S1, S2, W8）— 引入回调 trait 或返回结构化结果，让 CLI 和 GUI 各自处理展示。这是最关键的架构修复。

2. **🔴 修复 macOS shell 补全中的 `grep -oP`**（S6）— 直接影响用户体验，macOS 用户补全完全失效。

3. **🟡 消除 import.rs / switch.rs 的代码重复**（W13, W14）— 提取共享的 worktree 创建函数，消除 ~140 行重复。

4. **🟡 修复 `RepoRef::to_absolute_path` 类型签名**（S3）— 接受 `&HashMap<String, HostConfig>` 让编译器防止误用。

5. **🟡 将 wtp-gui 从 workspace 默认构建中排除**（S7）— 避免编译未完成的 crate 浪费 CI 时间。

---

*审计时间: 2026-03-28 | 审计员: code-reviewer*
