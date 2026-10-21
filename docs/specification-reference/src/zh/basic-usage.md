# 基本用法

完成[安装](installation.md)后，将仓库的 `rust-toolchain.toml` 复制到项目包的根目录，并在 `Cargo.toml` 中添加 `trust2-contract`：

```toml
[dependencies]
trust2-contract = { path = "/path/to/trust2-contract/trust2-contract" }
```

将路径替换为本地源码中 `trust2-contract` 库所在的目录。

## 编写契约

导入契约标注，并将它们添加到函数上：

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

前置条件要求调用者传入的 `x` 小于 16。后置条件将返回值命名为 `result`，并声明它不小于 `x`。

这个函数既可以放在库中，也可以放在可执行程序中。

## 在库中使用

将上面的代码放入 `src/lib.rs`，在包的根目录执行：

```sh
cargo verify --prover alt-ergo -- --lib
```

验证器在前置条件成立的假设下检查函数，不需要额外编写 `main` 函数。

## 在可执行程序中使用

将同样的代码放入 `src/main.rs`，并添加程序入口：

```rust,ignore
fn main() {
    let _ = square(4);
}
```

然后执行：

```sh
cargo verify --prover alt-ergo -- --bin my-app
```

将 `my-app` 替换为可执行目标的名称，默认就是 `Cargo.toml` 中的包名。这里 `main` 传入的 `4` 满足 `square` 的前置条件。

两条命令都使用安装时配置的 Alt-Ergo 证明器。`--` 后的参数用于选择 Cargo 目标：`--lib` 选择库，`--bin` 选择可执行程序。命令在包目录中执行；工作区成员也在各自的包目录中执行。

正常运行可执行程序使用 `cargo run`。普通构建会移除契约标注，静态检查契约则使用 `cargo verify`。

前置条件与后置条件的更多用法见[函数契约](function-contracts.md)。
