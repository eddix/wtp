# wtp - Workspace with git workTree for Polyrepo

[English](README.md)

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个基于 [git worktree](https://git-scm.com/docs/git-worktree) 将多个独立仓库组装为**动态 monorepo** 工作空间的 CLI 工具。

## 为什么需要 wtp？

Monorepo 是管理关联代码库的流行方案，但它并非万能：

- **过多的上下文反而影响 AI 表现。** 当你让 AI 优化"取消订单"的交互体验时，它可能会把用户在手机端的操作与同一仓库里商家工作台的同名功能混为一谈。更精确的、围绕任务的上下文才能带来更好的结果。
- **有些场景天然不适合 monorepo。** 把一个遗留代码库中仅剩 5% 真正有用的代码迁移到新仓库时，不应该先把 100% 的遗留代码全倒进来再让 AI 清理——那太脏了。保持仓库独立、按需引入才是更干净的路径。
- **Monorepo 的边界永远无法覆盖所有人。** 对小公司而言，"所有代码放一个仓库"说得通。但在一个拥有数百个团队的大型组织中，没有任何单一 monorepo 能真正囊括你某项工作可能涉及的全部代码。

**wtp 提供了一种不同的思路：动态 monorepo（Dynamic Monorepo）。**

你的仓库保持独立——各自拥有自己的历史、CI 和所有权。但当某项任务需要同时跨多个仓库工作时，`wtp` 将它们即时组装成一个统一的工作空间。任务完成后，工作空间随之解散。没有永久耦合，没有结构妥协。

这带来了：

- **任务级别的上下文** — 只包含与当前工作相关的仓库，不多不少
- **零迁移成本** — 无需重构现有仓库
- **灵活的边界** — 工作空间可以跨团队、跨组织、甚至跨 Git 托管平台
- **完全兼容 git** — 基于 [git worktree](https://git-scm.com/docs/git-worktree) 构建，每个仓库仍然是标准的 git 仓库

## 安装

### 前置要求

- Rust 1.90+（需要 Rust 2024 edition 支持）
- Git 2.30+

### 从源码安装

```bash
git clone https://github.com/eddix/wtp
cd wtp
cargo install --path wtp-cli
```

## 快速开始

```bash
# 为你的功能创建一个工作空间
wtp create feature-x

# 将当前仓库加入工作空间
# 这会创建一个名为 "feature-x" 的 worktree 和分支
cd ~/projects/my-repo
wtp switch feature-x

# 将另一个仓库也加入同一工作空间
cd ~/projects/another-repo
wtp switch feature-x

# 跳转到工作空间目录（需要 shell 集成，见下文）
wtp cd feature-x

# 或者在工作空间目录内导入仓库
cd ~/.wtp/workspaces/feature-x
wtp import company/project

# 查看工作空间内所有 worktree 的状态
wtp status
```

## Shell 集成

### Shell Wrapper（`wtp cd`）

要启用 `wtp cd`，请在 `.zshrc` 或 `.bashrc` 中添加：

```bash
eval "$(wtp shell-init)"
```

**工作原理：**
1. Shell wrapper 创建一个临时文件并设置 `WTP_DIRECTIVE_FILE` 环境变量
2. `wtp cd` 将 `cd '/path/to/workspace'` 命令写入该文件
3. wtp 退出后，wrapper 执行该文件，从而改变父 shell 的目录

### Shell 补全

使用 `wtp completions` 生成 Tab 补全脚本：

```bash
# Zsh（添加到 .zshrc）
eval "$(wtp completions zsh)"

# Bash（添加到 .bashrc）
eval "$(wtp completions bash)"

# Fish（添加到 config.fish）
wtp completions fish | source
```

补全支持子命令、flag、工作空间名称（动态）、host 别名（动态）以及文件路径。

## 配置

### 配置文件位置（优先级从高到低）

`wtp` 按以下顺序查找配置文件（使用第一个找到的）：

1. `~/.wtp.toml`
2. `~/.wtp/config.toml`
3. `~/.config/wtp/config.toml`

如果存在多个配置文件，会显示警告并提示当前使用的是哪个。

### 配置示例

```toml
# 工作空间设置
workspace_root = "~/.wtp/workspaces"  # 所有工作空间的默认位置

# Host 别名 - 将短名称映射到代码根目录
[hosts.gh]
root = "~/codes/github.com"

[hosts.gl]
root = "~/codes/gitlab.company.internal"

[hosts.bb]
root = "~/codes/bitbucket.org"

# 未指定时使用的默认 host
default_host = "gh"

# 显示偏好
[display]
# 在 `wtp ls --long` 中给 repo 名上色，颜色由 repo 名哈希得到且保持稳定
#（同一个 repo 永远同一种颜色）。
#   "auto"   - 输出为终端时上色（默认）
#   "always" - 即使被管道/重定向也上色
#   "never"  - 关闭 repo 上色
repo_colors = "auto"
```

### Repo 颜色

在 `wtp ls --long` 里，每个 repo 名会被染上一个由名字哈希得到的稳定颜色。同一个
repo 在不同 workspace 里永远是同一种颜色，于是你最常打交道的那几个 repo 一眼就能
认出来。通过 `[display].repo_colors`（`auto` / `always` / `never`）控制；当输出不是
终端或设置了 `NO_COLOR` 时会自动去色。

### Hooks

wtp 支持在工作空间生命周期事件上运行自定义脚本，适用于为工作空间初始化标准配置、工具或文档。

#### On-Create Hook

`on_create` hook 在新工作空间创建后运行。在配置文件中设置：

```toml
[hooks]
on_create = "~/.wtp/hooks/on-create.sh"
```

Hook 脚本可以使用以下环境变量：

| 变量 | 说明 | 示例 |
|------|------|------|
| `WTP_WORKSPACE_NAME` | 创建的工作空间名称 | `my-feature` |
| `WTP_WORKSPACE_PATH` | 工作空间目录的完整路径 | `/home/user/.wtp/workspaces/my-feature` |

**Hook 脚本示例**（`~/.wtp/hooks/on-create.sh`）：

```bash
#!/bin/bash
echo "Initializing workspace: $WTP_WORKSPACE_NAME"

# 在 workspace 根写 CLAUDE.md / AGENTS.md，让 coding agent 把这个目录
# 当作工作根，而不是钻进某个子 worktree。
cat > "$WTP_WORKSPACE_PATH/CLAUDE.md" <<EOF
# wtp workspace: $WTP_WORKSPACE_NAME

本目录是一个 **wtp workspace** —— 每个子目录都是为同一个任务 check
out 的独立 \`git worktree\`。把本目录视为工作根，仅在执行 repo 内部
命令时再 \`cd\` 到对应子目录。
EOF

# 同一份内容复制给 Codex 等其他 agent。
cp "$WTP_WORKSPACE_PATH/CLAUDE.md" "$WTP_WORKSPACE_PATH/AGENTS.md"

# 复制规范编码配置（示例）
# cp ~/.templates/spec-coding.toml "$WTP_WORKSPACE_PATH/.spec.toml"

echo "Workspace initialized!"
```

设置脚本为可执行：
```bash
chmod +x ~/.wtp/hooks/on-create.sh
```

**注意事项：**
- 使用 `wtp create <name> --no-hook` 跳过 hook
- Hook 失败不会阻止工作空间创建（会显示警告）
- Hook 的标准输出会显示在终端
- 在 Unix 系统上，脚本必须有执行权限

## 命令

### `wtp ls` - 列出工作空间

```bash
# 列出所有工作空间
wtp ls

# 详细列表（显示各 repo 的分支与状态）
wtp ls --long

# 只输出名字，每行一个（便于脚本与补全）
wtp ls --short

# 只列出"包含名字匹配 PATTERN 的 repo"的工作空间
#（大小写不敏感子串）。便于按 repo 命名空间筛选，
# 例如找出所有涉及 i18n_* 的工作空间。
wtp ls --grep i18n

# 可与任意输出格式组合
wtp ls --long --grep i18n
```

输出：
```
main
feature-x
hotfix-123
```

所有工作空间存储在 `workspace_root`（默认：`~/.wtp/workspaces`）。

### `wtp create <NAME>` - 创建工作空间

```bash
wtp create my-feature

# 跳过 on_create hook（如果已配置）
wtp create my-feature --no-hook
```

在 `<workspace_root>/<NAME>` 创建新的工作空间目录。如果配置了 `on_create` hook，将在创建后执行。

**注意：** 该命令会输出路径，但无法改变当前 shell 的目录。你需要手动 `cd`：

```bash
cd $(wtp create my-feature 2>&1 | grep "Created" | awk '{print $NF}')
```

### `wtp rm <NAME>` - 删除工作空间

```bash
# 删除工作空间（先逐一移除所有 worktree）
wtp rm my-feature

# 即使有未提交的更改也强制删除
wtp rm my-feature --force
```

该命令会：
1. 通过 `git worktree remove` 逐一移除所有 worktree（显示进度）
2. 检查工作空间目录是否有残留文件
3. 如果目录干净则删除，否则需要 `--force`

如果任何 worktree 有未提交的更改，命令会停止并列出（除非使用 `--force`）。

### `wtp import` - 导入仓库到当前工作空间

将外部 git 仓库导入到你当前所在的工作空间。你必须先 `cd` 进入工作空间目录。支持普通和 bare git 仓库。

```bash
# 使用默认 host 导入仓库（如果已配置）
cd ~/.wtp/workspaces/feature-x
wtp import company/project

# 使用指定的 host 别名导入
wtp import company/project -H gh

# 使用完整仓库路径导入
wtp import --repo ~/projects/my-repo

# 指定分支名（默认使用工作空间名称）
wtp import company/project -b feature-xyz

# 指定新分支的基准
wtp import company/project -B main

# 在当前所在分支上叠一层（见下文「Stacked Worktree」）。
# 在 worktree 目录内运行时 PATH 可省略，自动推导当前仓库
wtp import -b feature-xyz-2 --parent feature-xyz

# 交互模式：不带参数运行，模糊搜索选择仓库
wtp import
```

**仓库路径解析规则：**

1. 如果提供了 `--repo`：直接使用绝对路径
2. 如果提供了 `-H` / `--host`：`<host_root>/<path>`
3. 如果配置了 `default_host`：使用默认 host
4. 其他情况：视为绝对/相对文件系统路径

**工作空间检测：**

命令从当前目录开始向上查找 `.wtp` 目录来确定工作空间。如果不在工作空间内，会报错。

### `wtp eject` - 从工作空间移除 worktree

通过 `git worktree remove` 从当前工作空间移除仓库的 worktree。

```bash
# 移除指定仓库（必须在工作空间目录内）
wtp eject my-repo

# 即使有未提交的更改也强制移除
wtp eject my-repo --force

# 交互模式：不带参数运行，从工作空间仓库中选择
wtp eject
```

命令从当前目录检测工作空间。移除后，worktree 记录会从 `.wtp/worktree.toml` 中删除。

### `wtp restack` - 将 stack 各层 rebase 到各自 parent 上

下层分支前进后，级联重堆上面的层（见下文「Stacked Worktree」）。

```bash
# 在某层 worktree 目录内运行：只重堆该层所在的整条链
cd ~/.wtp/workspaces/feature-x/project@feature-xyz-2
wtp restack

# 在工作空间根运行：重堆工作空间内的所有链
cd ~/.wtp/workspaces/feature-x
wtp restack
```

行为：
- **预检快速失败**：任何一层有未提交更改、进行中的 rebase、目录缺失或
  parent 无法解析，都会在开始前一次性列出并拒绝执行
- 每层在自己的 worktree 里执行 `git rebase --onto <parent> <fork_point>`，
  只重放该层独有的 commit——底层被 squash merge 后也能干净移植
- **无状态、幂等**：已经基于 parent 的层自动跳过；冲突时停下并打印
  worktree 路径、冲突文件和处理指引——解决后 `git rebase --continue`，
  再跑一次 `wtp restack` 就会从中断处继续
- wtp 永不 push：结束时列出被改写的分支和
  `git push --force-with-lease` 命令模板

### `wtp retarget` - 改挂 worktree 的 stack parent

只改元数据，不动 git 历史。典型场景：stack 底层的 PR 合入后，它的子层
需要改挂到 `main` 上。

```bash
# 在某层 worktree 目录内运行：改挂当前层
wtp retarget main

# 在工作空间任意位置：显式指定 worktree
wtp retarget project@feature-xyz-2 main

# 然后应用变更
wtp restack
```

自指、不存在的 ref、成环都会被拒绝。已记录的 fork point 会保留——
这正是 squash merge 之后 restack 能干净通过的关键。

### `wtp switch <WORKSPACE>` - 将当前仓库加入工作空间

通过创建 worktree 将当前 git 仓库添加到工作空间。

```bash
# 将当前仓库切换到已有工作空间
wtp switch my-feature

# 切换并在不存在时创建工作空间
wtp switch --create my-feature

# 指定分支名
wtp switch my-feature --branch custom-branch

# 指定基准分支
wtp switch my-feature --base develop
```

该命令会：
1. 检测当前 git 仓库
2. 创建/使用指定分支
3. 在目标工作空间中创建 worktree
4. 在工作空间元数据中记录 worktree

**工作空间处理：**
- 不带 `--create`：工作空间必须已存在
- 带 `--create`：不存在时自动创建

**Host 匹配：** 记录 worktree 时，`wtp` 会尝试将仓库路径与配置的 host 别名匹配，存储相对引用而非绝对路径。

### `wtp status` - 查看工作空间状态

显示当前工作空间中所有 worktree 的状态。必须在工作空间目录内运行。

```bash
# 查看当前工作空间状态
wtp status

# 详细状态（包含远程跟踪、最后提交信息）
wtp status --long
```

### `wtp cd <WORKSPACE>` - 跳转到工作空间目录

```bash
# 跳转到工作空间目录
wtp cd my-feature
```

**注意：** 该命令需要 shell 集成。未启用时会显示：
```
Error: wtp cd requires shell integration
```

### `wtp host` - 管理 Host 别名

Host 别名将短名称映射到代码根目录，方便引用仓库。

```bash
# 添加 host 别名
wtp host add gh ~/codes/github.com

# 列出已配置的 host
wtp host ls

# 设置默认 host
wtp host set-default gh

# 删除 host 别名
wtp host rm gh

# 取消默认 host
wtp host set-default none
```

**使用示例：**

```bash
# 1. 添加 host 别名
wtp host add gh ~/codes/github.com
wtp host add gl ~/codes/gitlab.company.com

# 2. 设置默认
wtp host set-default gh

# 3. 现在可以使用短路径了
wtp import mycompany/project
# 解析为: ~/codes/github.com/mycompany/project
```

**配置：**

Host 存储在 `~/.wtp/config.toml` 中：

```toml
[hosts.gh]
root = "/home/user/codes/github.com"

[hosts.gl]
root = "/home/user/codes/gitlab.company.com"

default_host = "gh"
```

### 安全围栏

wtp 内置了**围栏**机制，防止意外在 `workspace_root` 之外进行文件操作。如果你试图：
- 在 `workspace_root` 之外创建工作空间
- 导入/切换到边界之外的工作空间

会看到如下警告：
```
Warning: Workspace 'xxx' is outside workspace_root: /Users/you/.wtp/workspaces
Target path: /some/outside/path
Are you sure you want to proceed? [y/N]
```

这可以保护你的系统文件不被 wtp 命令意外修改。

## 核心概念

### 工作空间（Workspace）

**工作空间**是来自不同仓库的 worktree 的逻辑集合。所有工作空间存储在 `workspace_root`（默认：`~/.wtp/workspaces`）。

每个工作空间包含：
- `.wtp/worktree.toml` - 所有 worktree 的元数据
- 各仓库 worktree 的子目录

### Worktree 布局

Worktree 的组织方式为：
```
<workspace_root>/<workspace_name>/<repo_slug>/
```

例如：
```
~/.wtp/workspaces/feature-x/
├── my-project/             # "my-project" 仓库的 worktree
└── another-project/        # "another-project" 仓库的 worktree
```

**约束：**
- 默认情况下，每个仓库在每个工作空间中只有**一个** worktree，目录名为仓库
  slug（仓库路径的最后一段）
- 如果尝试重复添加，会得到如下错误：
  ```
  Error: Repository 'my-project' is already in this workspace with branch 'feature-x'.
  To add another branch of the same repository, re-run with -b <branch> and --with-branch-name.
  ```

### 同一仓库的多个分支

如果需要在同一个工作空间中并排操作一个仓库的多个分支（例如把同一个修改应用到
多个 release 分支），给 `wtp import` 或 `wtp switch` 加上 `--with-branch-name`，
worktree 目录会命名为 `<repo_slug>@<branch>`：

```bash
wtp import company/project -b release-area-a-dev                    # project/
wtp import company/project -b release-area-b-dev --with-branch-name # project@release-area-b-dev/
```

```
~/.wtp/workspaces/releases/
├── project/                        # 分支：release-area-a-dev
└── project@release-area-b-dev/     # 分支：release-area-b-dev
```

注意：
- 分支名中的 `/` 等分隔符在目录名中会被转义（`feature/x` → `project@feature_x`）
- `wtp eject` 可以用 worktree 目录名精确指定要弹出的 worktree
  （如 `wtp eject project@release-area-b-dev`）；裸仓库 slug 匹配到多个
  worktree 时会报歧义错误
- 同一仓库的所有 worktree 共享同一套 git refs、stash 和 config——这是
  `git worktree` 的标准行为

### Stacked Worktree（纵向开发）

除了跨仓库的横向开发，wtp 还支持在**同一个仓库内**做 stacked PR 式的
纵向开发：一条相互依赖的分支链，每层一个独立的 worktree 目录。

```bash
cd ~/.wtp/workspaces/feature-x/project        # 站在 feat-1 上
wtp import -b feat-2 --parent feat-1          # 往上叠 feat-2
cd ../project@feat-2
wtp import -b feat-3 --parent feat-2          # 再叠 feat-3
```

```
~/.wtp/workspaces/feature-x/
├── project/            # feat-1（stack 底层）
├── project@feat-2/     # feat-2，parent: feat-1
└── project@feat-3/     # feat-3，parent: feat-2
```

`wtp status` 会把链画成树，并显示每层相对 parent 的分歧
（`↑` 本层独有 commit 数，`↓` parent 上还没重堆进来的 commit 数）：

```
REPOSITORY      BRANCH           STATUS
project         feat-1           ✓ clean
project         └ feat-2 ↑2      ✓ clean
project           └ feat-3 ↑1    ✓ clean
```

各部分的职责：

- `--parent` 在 `.wtp/worktree.toml` 里记录链的边和 **fork point**
  （本层从 parent 的哪个 commit 上切出来）
- `wtp restack` 级联重堆，每层只重放自己独有的 commit
- `wtp retarget` 在底层合入后改挂边
- parent 可以是**任意 ref**，不限于另一层：底层可以挂 `origin/main`
  追踪主干；parent 分支合并删除后，孤儿层可以改挂到 `main`

相比单 checkout 里切分支（`git rebase --update-refs`、Graphite），
每层一个目录的好处：底层在等 review 时你继续写上层，review 意见来了
直接 `cd` 下去改——不用 stash、不用切换上下文；每层还能各自跑一个
coding agent。

完整设计与动机见
[`docs/design/stacked-worktree.md`](docs/design/stacked-worktree.md)。
wtp 刻意不碰 forge——不建 PR、不改 PR base、不 push，请配合
`gh`/`glab` 等 forge CLI 使用。

### Host 别名

Host 别名让你可以用短名称代替完整路径：

```toml
[hosts.gh]
root = "~/codes/github.com"
```

这样就不需要：
```bash
wtp import ~/codes/github.com/company/project
```

而是可以：
```bash
wtp import company/project -H gh
# 或者配置了 default_host 后：
wtp import company/project
```

## 分支管理

### 创建新分支

默认情况下，`wtp import` 和 `wtp switch` 会创建与工作空间同名的分支：

```bash
wtp create feature-x
wtp switch feature-x    # 从当前 HEAD 创建 "feature-x" 分支
```

### 使用已有分支

如果分支已存在且未在其他 worktree 中检出：

```bash
wtp import company/project -b existing-branch
```

这将检出已有分支而非创建新分支。

### 分支冲突

Git worktree 有一个约束：**一个分支同一时间只能在一个 worktree 中检出**。

如果尝试添加一个已被检出的分支：

```
Error: Branch 'feature-x' is already checked out in another worktree: my-project/feature-x
```

**解决方法：**
1. 使用不同的分支名：`wtp import ... -b feature-x-2`
2. 先移除已有的 worktree
3. 使用不同的工作空间

## 故障排除

### "Not in a workspace directory"

`wtp import`、`wtp eject` 和 `wtp status` 等命令从当前目录检测工作空间：

```
Error: Not in a workspace directory.
Run this command from within a workspace directory.
```

请先 `cd` 进入工作空间目录。

### "Branch already checked out"

参见上方[分支冲突](#分支冲突)。

### 多配置文件警告

```
Warning: Multiple config files found: ~/.wtp.toml, ~/.wtp/config.toml. Using ~/.wtp.toml
```

请将配置合并到一个文件中，删除其他文件。

### Git worktree 常见错误

1. **"already checked out"** - 分支正在被其他 worktree 使用
2. **"is not a valid repository"** - 仓库路径不存在或不是 git 仓库
3. **"is locked"** - 之前的 git 操作被中断，可能需要手动清理

## 开发

### 构建

```bash
cargo build --release
```

### 运行测试

```bash
cargo test
```

### 项目结构

```
wtp-core/src/            # 核心业务逻辑（UI 无关）
├── lib.rs               # 公共 API 导出
├── config.rs            # 配置管理
├── error.rs             # 错误类型
├── fence.rs             # 安全围栏
├── git.rs               # Git 命令封装
├── workspace.rs         # 工作空间管理
└── worktree.rs          # Worktree 数据模型

wtp-cli/src/             # CLI 应用
├── main.rs              # 入口
└── cli/                 # CLI 子命令
    ├── mod.rs           # CLI 入口和帮助系统
    ├── cd.rs
    ├── completions.rs   # Shell 补全生成
    ├── create.rs
    ├── eject.rs         # 从工作空间移除 worktree
    ├── fuzzy.rs         # 模糊搜索集成
    ├── git_status_fmt.rs # Git 状态格式化扩展
    ├── host.rs          # Host 别名管理
    ├── import.rs
    ├── ls.rs
    ├── remove.rs
    ├── shell_init.rs
    ├── status.rs
    ├── switch.rs
    └── theme.rs         # 帮助输出的统一样式

wtp-gui/                 # GUI 应用（脚手架，基于 GPUI）
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)。

## 贡献

欢迎贡献！请随时提交 Pull Request。

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=eddix/wtp&type=Date)](https://star-history.com/#eddix/wtp&Date)
