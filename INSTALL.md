# wtp 安装指南

本文档用于指导 code agent 为用户完成 wtp 的安装。

## 安装步骤

### 第零步：获取代码

如果用户还没有 clone 仓库，先执行：

```bash
git clone https://github.com/eddix/wtp.git
cd wtp
```

如果用户已经有了本地仓库，跳过此步骤。

### 第一步：检查 Rust 环境

首先检查系统是否已经安装了 Rust（要求 1.90 或更高版本）：

```bash
rustc --version
cargo --version
```

如果两个命令都能正常显示版本信息，且版本满足要求，可以跳过第二步，直接进入第三步。

### 第二步：安装 Rust 环境

如果系统未安装 Rust，需要先安装 rustup。

**请询问用户使用哪种方式安装 rustup：**

1. **通过 Homebrew 安装**（推荐 macOS 用户）
2. **使用 rustup 官方脚本安装**

#### 选项 1：通过 Homebrew 安装

```bash
brew install rustup
```

这条命令会安装 rustup（以前叫 rustup-init），本身不直接装 Rust，而是提供 Rust 工具链管理器。

#### 选项 2：使用 rustup 官方脚本安装

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 初始化 rustup（安装 Rust + Cargo）

无论选择哪种方式安装 rustup，安装完成后都需要运行：

```bash
rustup-init
```

交互流程里直接按回车用默认选项（默认装 stable toolchain，并加好 PATH）。

成功后会安装好：

- `rustc`：Rust 编译器
- `cargo`：包管理/构建工具
- `rustup`：工具链管理器本体

安装完成后，需要重新加载环境变量：

```bash
source $HOME/.cargo/env
```

### 第三步：安装 wtp

**请询问用户使用哪种方式安装：**

1. **使用 cargo install 安装**（推荐，自动安装到 Cargo bin 目录）
2. **手动编译并安装到自定义位置**

#### 选项 1：使用 cargo install 安装（推荐）

```bash
cargo install --path .
```

这会将 `wtp` 安装到 `~/.cargo/bin/`，通常该目录已在 PATH 中，无需额外配置。

#### 选项 2：手动编译

```bash
cargo build --release
```

编译完成后，二进制文件位于 `target/release/wtp`。

**请询问用户将 wtp 放在什么位置：**

1. 默认位置：`~/.local/bin/`（推荐）
2. 用户自定义位置

##### 安装到默认位置 ~/.local/bin/

```bash
mkdir -p ~/.local/bin
cp target/release/wtp ~/.local/bin/
```

检查 `~/.local/bin` 是否在 PATH 中：

```bash
echo $PATH | grep -q "$HOME/.local/bin"
```

如果不在 PATH 中，需要添加到 PATH。根据用户使用的 shell 进行操作：

**对于 bash 用户：**

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

**对于 zsh 用户：**

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

##### 安装到用户自定义位置

请用户提供具体路径，然后执行：

```bash
mkdir -p /用户/指定/路径
cp target/release/wtp /用户/指定/路径/
```

确保该路径已在用户的 PATH 中，或者提醒用户将该路径添加到 PATH。

### 第四步：配置 Shell 集成（推荐）

为了使 `wtp cd` 命令能够切换当前 shell 的目录，需要添加 shell 集成。

根据用户使用的 shell，将以下内容添加到对应配置文件：

**对于 zsh 用户**（添加到 `~/.zshrc`）：

```bash
eval "$(wtp shell-init)"
```

**对于 bash 用户**（添加到 `~/.bashrc`）：

```bash
eval "$(wtp shell-init)"
```

添加后重新加载配置：

```bash
source ~/.zshrc  # 或 source ~/.bashrc
```

### 第五步：配置 Shell 补全（可选）

wtp 支持 zsh、bash、fish 的 Tab 补全。根据用户使用的 shell 进行配置：

**zsh**（添加到 `~/.zshrc`）：

```bash
eval "$(wtp completions zsh)"
```

**bash**（添加到 `~/.bashrc`）：

```bash
eval "$(wtp completions bash)"
```

**fish**（添加到 `config.fish`）：

```bash
wtp completions fish | source
```

### 第六步：验证安装

安装完成后，运行以下命令验证安装是否成功：

```bash
wtp --help
```

如果显示帮助信息，说明安装成功！

**告诉用户：**

> wtp 已成功安装！你可以运行 `wtp --help` 查看所有可用命令。建议参考 README.md 完成初始配置，例如设置 workspace_root 和 host aliases。
