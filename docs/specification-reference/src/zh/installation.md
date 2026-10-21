# 安装

在 Linux AMD64、macOS AMD64 或 macOS ARM64 上从源码构建 trust2-contract。下面创建本地 OCaml 环境，并先使用 Alt-Ergo 作为证明器。命令适用于 Bash 或 Zsh。

## 1. 准备系统工具

安装 Git、C/C++ 构建工具链和 opam。

Debian 或 Ubuntu：

```sh
sudo apt update
sudo apt install git curl build-essential pkg-config libgmp-dev opam
```

macOS：如果尚未安装 Command Line Tools，先安装它，再通过 [Homebrew](https://brew.sh/) 安装 opam：

```sh
xcode-select --install
brew install git opam pkg-config
```

其他发行版参考 [opam 安装说明](https://opam.ocaml.org/doc/Install.html)。解析依赖时，opam 可能提示安装额外的系统软件包。

## 2. 获取源码

```sh
git clone https://github.com/OpenSourceVerif/trust2-contract.git
cd trust2-contract
```

这里是仓库根目录，包含 `Cargo.toml`、`rust-toolchain.toml` 和 `trust2-contract.opam`。除非另行说明，后续配置命令均在这里执行。请保留这份源码，当前契约库通过路径依赖接入项目。

## 3. 配置 Rust

如果尚未安装 rustup，使用 [Rust 官方安装器](https://rust-lang.org/tools/install/)：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
```

在仓库根目录检查选中的工具链：

```sh
rustc --version
rustup show active-toolchain
```

Rustup 会读取仓库的 `rust-toolchain.toml`，在需要时下载指定工具链和组件。当前文件固定为 `nightly-2026-06-01`，并包含 `rustc-dev`、`llvm-tools` 和 `miri`。前端依赖 Rust 编译器内部接口，因此应使用当前源码对应的工具链文件。

## 4. 配置 OCaml 环境

首次使用 opam 时，先初始化，并暂不创建全局 switch：

```sh
opam init --bare
```

按仓库的开发配置创建本地 OCaml switch，激活环境，再安装项目依赖：

```sh
opam switch create . 5.3.0
eval $(opam env)
opam install . --deps-only
```

如果这份源码已经有本地 switch，直接复用即可。依赖清单固定了 Why3find 的一个开发版本；通过 `opam install . --deps-only` 安装，以遵循该版本配置。

检查当前环境能否找到 Why3find：

```sh
command -v why3find
```

打开新终端后，先回到仓库根目录执行 `eval $(opam env)`，再运行验证器。这会将本地 switch 的可执行文件加入 `PATH`，随后进入示例目录也能继续使用。switch 管理方式见 [opam 使用说明](https://opam.ocaml.org/doc/Usage.html)。

## 5. 安装证明器

执行验证除了需要 Why3find，还需要外部证明器。可以先在同一个 switch 中安装 [Alt-Ergo](https://opam.ocaml.org/packages/alt-ergo/)，再让 Why3 检测它：

```sh
opam install alt-ergo
why3 config detect
why3 config list-provers
```

确认检测结果中包含 Alt-Ergo。之后安装其他证明器时，应重新运行检测，详见 [Why3 配置说明](https://why3.org/doc/manpages.html#the-config-command)。

验证器的默认证明器列表为 Alt-Ergo、CVC5 和 Z3。下面的示例显式选择 Alt-Ergo，只请求这份安装流程中配置的证明器。安装并检测其他证明器后，可通过 `--prover` 选择，例如 `--prover alt-ergo,z3`。

## 6. 安装验证器

在仓库根目录执行：

```sh
cargo install --path ./cargo-verify
cargo verify --help
```

Cargo 通常将 `cargo-verify` 安装到 `~/.cargo/bin`。该目录和当前 opam switch 的 `bin` 目录都需要保留在 `PATH` 中。

## 7. 检查安装结果

运行仓库自带的示例：

```sh
cd examples/readme
cargo verify --prover alt-ergo
```

这一步一起检查 Rust 前端、规约转换和证明器调用。成功时应以 `Verified` 摘要结束，并正常退出；只运行 `--help` 仅能确认命令已安装。

接下来阅读[基本用法](basic-usage.md)，将契约库接入自己的包。

## 常见问题

- **找不到 `cargo verify`：** 检查 Cargo 安装目录是否在 `PATH` 中；默认 rustup 配置下，可重新加载 `. "$HOME/.cargo/env"`。
- **找不到 `why3find`：** 在运行验证的终端中，回到仓库根目录执行 `eval $(opam env)`，激活本地 switch。
- **没有匹配的证明器：** 检查 `why3 config list-provers`，确认可执行文件位于 `PATH` 中，并重新执行 `why3 config detect`。
- **Rust 编译器或组件不匹配：** 使用源码自带的 `rust-toolchain.toml`，不要替换为其他 nightly。
