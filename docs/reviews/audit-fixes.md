# FIX_REVIEW.md — Audit Fix Review Report

**审查日期**: 2026-03-28
**审查方式**: Codex GPT-5.4 (read-only, xhigh) + cargo check/test 验证
**审查范围**: wtp-core + wtp-cli（跳过 wtp-gui）

---

## 编译与测试

| 检查项 | 结果 |
|--------|------|
| `cargo check --workspace` | PASS (仅 wtp-gui 3 个 dead_code 警告) |
| `cargo test --workspace` | PASS (5 passed, 0 failed) |

---

## 总览

| 状态 | 数量 |
|------|------|
| FIXED | 25 |
| PARTIAL | 7 |
| NOT_FIXED (按设计跳过) | 5 |
| **Blocker** | **1** |
| **Warning** | **10** |
| **Info** | **2** |

---

## 1. Blockers (1)

### H1. fence.rs Symlink TOCTOU — PARTIAL

**问题**: `safe_create_dir_all` 已实现并替换了 `std::fs::create_dir_all`，但仍有残余风险：

1. `is_within_boundary` (fence.rs:126) 没有对祖先路径做 symlink 遍历检查
2. `safe_create_dir_all` 在单次祖先检查后，逐级 `std::fs::create_dir(dir)` — 在 check 和 mkdir 之间仍有 TOCTOU 窗口（攻击者可在此窗口替换目录为 symlink）
3. 虽然 mkdir 后有 `symlink_metadata` 后检查 (fence.rs:245)，但已经创建的外部目录无法回滚
4. fence 测试 (fence.rs:350-406) 仅覆盖词法路径遍历，未覆盖 symlink 竞态场景
5. `validate_within_boundary` 的非存在路径行为继承了同样的弱点

**建议**: 考虑使用 `openat(2)` / `mkdirat(2)` + `O_NOFOLLOW` 语义逐级创建，彻底消除 TOCTOU 窗口。或至少在 `is_within_boundary` 中添加祖先 symlink 遍历检查。

---

## 2. Warnings (10)

### H3. remove 路径遍历 — PARTIAL

**证据**: 边界校验已添加 (remove.rs:45, remove.rs:81)，但越界条目仅 `continue` 跳过 (remove.rs:55, remove.rs:90)，不是 abort 整批操作。同时 metadata 仍在 remove.rs:139 批量删除。FIX_DESIGN.md 要求"任何 entry 越界则中止整个删除"，但实际实现只是跳过。

**建议**: 将 `continue` 改为 `bail!` 或在循环前先做一轮全量校验。

### L2. config symlink / atomic save — PARTIAL

**证据**: symlink 检查已添加 (config.rs:108)，但 save 仍使用 `std::fs::write` (config.rs:124)，未实现 temp file + atomic rename。`tempfile` 未从 dev-dependencies 移到 dependencies。

**建议**: 添加 `tempfile` 到 normal deps，使用 `NamedTempFile::new_in` + `persist` 实现原子写入。

### P1. status --long 合并 git 查询 — PARTIAL

**证据**: `get_head_info` 已实现 (git.rs:126) 合并了 hash/subject/time 为一次 git 调用。但缺少 `get_full_status` / `FullGitStatus`，status.rs 仍然分别调用 `get_status` (status.rs:160) 和 `get_stash_count` (status.rs:185)。实际从 6 次降至约 4 次 git 调用。

**建议**: 实现 `get_full_status` 使用 `git status --porcelain=v2 --branch --show-stash` 进一步合并查询。

### P6. fence canonicalize 缓存 — PARTIAL

**证据**: `canonical_boundary` 字段已添加到 Fence struct (fence.rs:73)，但 `effective_canonical_boundary()` (fence.rs:99) 仍然每次调用 `self.boundary.canonicalize()`，未使用缓存字段。

**建议**: 让 `effective_canonical_boundary()` 直接返回 `&self.canonical_boundary`。

### S3. RepoRef::to_absolute_path — PARTIAL

**证据**: 签名已改为接受 `HostConfig` (worktree.rs:43)，但 host 不存在时仍 silently fallback 到 `PathBuf::from(path)` (worktree.rs:45)，无 `tracing::warn!`。

**建议**: 添加 `tracing::warn!("Host '{}' not found, treating path as relative", host)`。

### S5. 分支回退警告 — PARTIAL

**证据**: 回退不再静默（添加了 `eprintln!`），但未添加 `tracing::warn!` (import.rs:98, switch.rs:161)。FIX_DESIGN.md 设计同时包含 `tracing::warn!` 和 `eprintln!`。

**建议**: 补充 `tracing::warn!` 调用以支持结构化日志。

### P2. GitClient 全同步阻塞 — NOT_FIXED

**证据**: GitClient 仍全面使用 `std::process::Command` (git.rs:30, 181, 223, 252)。未添加 `spawn_blocking` 包装或 async 方法。

**说明**: FIX_DESIGN.md 将此列为渐进式改进，可推迟。

### P3. ls --long 串行探测 — NOT_FIXED

**证据**: ls.rs:44-116 仍然串行遍历所有 workspace。依赖 P2 提供 async 方法。

**说明**: FIX_DESIGN.md 明确依赖 P2，可推迟。

### P5/A10. scan_git_repos 复杂度 — NOT_FIXED

**证据**: git.rs:471-515 仍使用 `sort_by_file_name()` 和线性 `skip_prefixes.iter().any(...)` 前缀检查。

**说明**: 属于性能 Medium，非本轮必修。

### W16. git.get_status() 错误静默吞掉 — PARTIAL

**证据**: ls.rs 和 status.rs 已改善错误展示，但 remove.rs 的两个循环 (remove.rs:58, remove.rs:95) 仍然静默忽略 `get_status()` 失败。

---

## 3. Info (2)

### A8. to_string_lossy() 配置路径 — NOT_FIXED

**证据**: config.rs:42, 47, 53 仍使用 lossy UTF-8 转换，未添加文档注释说明限制。FIX_DESIGN.md 设计为"本轮仅添加注释"。

### P8. slug/display 临时分配 — NOT_FIXED

**证据**: worktree.rs:65 的 `slug()` 仍然每次分配，未添加 `slug_ref()` 借用版本。FIX_DESIGN.md 设计了 `slug_ref` 但未标注到哪个 Batch。

---

## 4. Confirmed FIXED (25)

| Issue | Status | 关键证据 |
|-------|--------|---------|
| H2 eject 路径遍历 | FIXED | `validate_within_boundary` 公开 (fence.rs:46)，eject.rs:62 调用 |
| M1 hook 相对路径 | FIXED | 相对路径解析为 config 目录 (config.rs:52) |
| M2 git 参数注入 | FIXED | `validate_git_ref` (git.rs:14) + `--` (git.rs:350) |
| M3 import 路径遍历 | FIXED | hosted 路径校验 (import.rs:173, 183) |
| M4 switch 参数注入 | FIXED | 通过 common.rs:76 共享逻辑，M2 覆盖 |
| L1 fence fail-open | FIXED | WorktreeToml::save 和 Manager::save 均 fail-closed (worktree.rs:144, 230) |
| P4 批量 TOML save | FIXED | `remove_many` (worktree.rs:272)，remove.rs:140 使用 |
| P7 detect_workspace 优化 | FIXED | 不再调用 scan_workspaces (workspace.rs:236-262) |
| S1 FenceConfirm trait | FIXED | trait + StdioConfirm (fence.rs:10)，Fence 持有可选确认器 (fence.rs:70) |
| S2 CreateResult | FIXED | 结构化返回 (workspace.rs:10)，CLI 处理 (create.rs:19) |
| S6 macOS completions | FIXED | `awk` 替换 `grep -oP` (completions.rs:164, 241) |
| W1 unwrap→expect | FIXED | config.rs:77 |
| W3 OnceLock Result | FIXED | init_global_fence 返回 Result (fence.rs:315) |
| W4 ahead/behind Option | FIXED | 返回 Option<(u32,u32)> (git.rs:369) |
| W6 count as u32 | FIXED | 使用 try_into 防截断 |
| W7 remove_by_slug 错误类型 | FIXED | 返回 crate::Result<bool> (worktree.rs:187) |
| W8 emoji 移除 | FIXED | 纯文本警告 (config.rs:74) |
| W9 IndexMap | FIXED | hosts 改为 IndexMap (config.rs:149) |
| W11 死代码 | FIXED | import.rs 冗余冲突检查已删除 |
| W13/W14 重复消除 | FIXED | common.rs 共享逻辑 |
| A2 run_git helper | FIXED | git.rs:30 |
| A3 Default version | FIXED | 手动 Default 委托 new() (worktree.rs:208) |
| A4 WorktreeToml re-export | FIXED | lib.rs:17 |
| A5 Hook/Uuid 错误变体 | FIXED | error.rs:37, 40 |
| A9 排序输出 | FIXED | list_workspaces 排序 |

---

## 5. API 兼容性评估

| 新增 API | 评估 |
|----------|------|
| `FenceConfirm: Send + Sync` trait | 合理，GUI 和测试可注入自定义实现 |
| `StdioConfirm` struct | 合理，CLI 默认实现 |
| `CreateResult { path, hook_output, hook_warning }` | 合理，替代库层打印 |
| `validate_within_boundary(parent, child) -> Result<PathBuf>` | 合理，但非存在路径行为继承 H1 弱点 |
| `get_head_info(repo_path) -> Result<(String, String, String)>` | 合理，合并 3 次 git 调用 |
| `remove_many(slugs: &[&str]) -> Result<usize>` | 合理，已在 remove.rs 中使用 |
| `validate_git_ref(value, param_name) -> Result<()>` | 合理，统一 git 参数校验 |

无 API 破坏性变更发现。新增的 trait 和 struct 均向后兼容。

---

## 6. 按设计跳过的项目

以下项目在 FIX_DESIGN.md 中明确标注为"本轮不修改"或依赖其他项：

- W5, W12, W15, W18, W19 — 代码风格/重构风险高
- A1, A6, A11, A12, A15, A16, A18, A19 — 影响小或需大改
- P2, P3 — 需 async 基础设施改造
- S7-S9, W20-W24, A20-A23 — GUI 相关，跳过

---

## 7. 结论

**总体评价**: 大部分修复正确实现，核心安全问题 (H2, M1-M4, L1) 和代码质量问题 (S1-S2, S6) 已修复。

**需要关注**:
- **1 个 Blocker**: H1 fence symlink TOCTOU 修复不完整，仍存在竞态窗口
- **10 个 Warning**: 其中 H3 的"跳过而非中止"行为偏离设计，L2 缺少 atomic write，P1/P6 修复不完全

**建议**:
1. 优先修复 H1 blocker — 至少在 `is_within_boundary` 添加祖先 symlink 检查
2. H3 remove 行为改为全量预校验+中止
3. L2 完成 atomic write
4. P1/P6 完成设计中的完整优化

---

*审查报告结束*
