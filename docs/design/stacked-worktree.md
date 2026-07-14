# Stacked Worktree（纵向开发）

> 状态：方案已定（2026-07-14 grill 共识），按 Phase 1/2/3 实施。

## 背景与术语

wtp 原生支持**横向开发**：一个 workspace 聚合多个仓库、每仓一个分支。本设计将其扩展到**纵向开发**：同一仓库的多个关联分支（stacked PR 式分步开发），每个分支一层、每层一个独立 worktree 目录。

在 wtp 中这套机制称为 **stacked worktree**——区别于传统 stacked branch 工具（Graphite、`git rebase --update-refs`）在单 checkout 内切换，wtp 的每层是独立目录：下层等 review 时上层继续写，review 意见来了直接进下层目录改，上层工作区不动；也天然适合多个 code agent 各守一层。

## 设计红线

- **永不碰 forge/远端**：不开 PR、不 retarget PR base、不执行 push。restack 完成后只提示需要 `git push --force-with-lease` 的分支清单。
- **不内嵌 AI**：冲突解决交给外部 agent/人，wtp 只保证中断信息结构清晰、命令幂等可重入。
- **不引入 workspace 类型**：链是 worktree 之间的关系属性，横向纵向可在同一 workspace 混用。

## 数据模型

`WorktreeEntry`（`.wtp/worktree.toml`）新增两个可选字段。serde 反序列化忽略未知字段，老版本 wtp 读新文件不受影响，version 不 bump：

| 字段 | 类型 | 语义 |
|---|---|---|
| `parent` | `Option<String>` | 链的边。任意 branch 名/ref 字符串。解析时**优先**匹配同 workspace 同 repo 的另一层（成链、进树状展示）；匹配不到则当普通 git ref（存在即可作 rebase 目标，不存在报 parent missing）。 |
| `parent_head` | `Option<String>` | fork-point。建层时记 parent 当时的 commit；每次 restack 成功后更新。 |

原 `base` 字段保留原语义（创建时起点的一次性快照），不复用、不迁移。

fork-point 的作用：restack 执行 `git rebase --onto <parent> <parent_head> <branch>`，只重放本层独有 commit。feat-1 被 squash merge 进 main 后，feat-2 retarget 到 main 再 restack 可干净通过（旧 commit 不会被重放）——这是 Graphite/jujutsu 的核心机制。

## 命令面（全平铺，无 `wtp stack` 命名空间）

### 建层：`wtp import -b <branch> --parent <ref>`

- 在某层 worktree 目录里执行可省 PATH：repo 推导为当前 worktree 的 repo。无 `--parent` 时 import 行为完全不变（省 PATH 仍走 fuzzy/报错路径）。
- `--parent` 隐含 `--with-branch-name`（目录自动 `<repo>@<branch>`）。
- 新分支从 parent HEAD 创建；`--parent` 与 `-B/--base` 互斥（clap `conflicts_with`）。
- 分支已存在时只挂关系，`parent_head` 取 `merge-base(branch, parent)`。
- 底层追 trunk：`--parent origin/main` 即可，无特例。普通 import 的层没有 parent，restack 永不触碰。

### 改挂：`wtp retarget [<worktree-dir>] <new-parent>`

- 层目录里跑：作用于当前层，单参数。workspace 根跑：双参数指定层。
- **只改元数据**，不动 git；打印提示 "run `wtp restack` to apply"。
- 保留旧 `parent_head` 不变——squash-merge 后精确移植正依赖它。
- 写入前环检测（沿 parent 链走，遇已访问层即拒绝）。

### 重堆：`wtp restack`

- 范围目录敏感：层目录里跑 = 当前层所在**整条链**（从根层到所有叶子）；workspace 根跑 = 所有链。
- 预检（fail fast，任何一项不过即拒绝开始并列出）：
  - 链上所有层 working tree 干净；
  - 无未完成的 rebase（`.git/rebase-merge` / `.git/rebase-apply` 存在即报告）。
- 拓扑序逐层进各自 worktree 执行 `git rebase --onto <parent> <parent_head>`；rebase 成功后更新该层 `parent_head` 为 parent 当前 commit。
- **幂等**：已 up-to-date 的层跳过；冲突即停，打印哪层目录、冲突文件、"解决后重跑 `wtp restack`"。无 `--continue` 状态机，重跑自动跳过已完成层。
- 全部完成后列出被改写的分支，提示 `git push --force-with-lease`，**不执行**。

### 展示：树状视图进现有 `wtp status`

- 同 repo 的链画成树；链内每层显示相对 parent 的 ahead/behind（复用 `get_ahead_behind`）。
- parent 解析不到（分支已删）显示 `parent missing`，附建议的 `wtp retarget` 命令。
- 非链 worktree 保持现有平铺展示。不加新命令、不进 `ls`、v1 不做 `--json`。

## 边界行为

- **eject 中间层不断链**：worktree 被 eject 后分支仍在 git 里，子层 parent 退化为普通 ref，仍可 restack。eject 时提示一句，不阻止。
- **删分支才断链**：status 显示 parent missing。
- `wtp rm` workspace 行为不变（分支留在原 repo）。

## 实施划分

| Phase | 内容 | 交付 |
|---|---|---|
| 1 | schema 两字段；`import --parent` 建层；status 树状 / ahead-behind / parent missing | 独立 commit 组 + integration tests |
| 2 | `wtp retarget`；环检测；eject 提示 | 同上 |
| 3 | `wtp restack`：预检、拓扑序、`--onto`、幂等中断、force-push 清单 | 同上 |
| 收尾 | README / README_CN / `skills/wtp-stacked-worktree` | 独立 commit |

## 明确不做

forge/PR 集成、`restack --continue` 状态机、wtp 内嵌 AI、`git merge-base --fork-point` 推断（依赖 reflog 不可靠）、status `--json`。
