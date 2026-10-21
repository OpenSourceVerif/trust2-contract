{{#rule contract.assertion}}

# 断言

{{#syntax Assertion}}

## 语义

`contract_assert!(P);` 要求布尔条件 `P` 在当前位置成立。它引入一个证明义务，证明时可以使用函数前置条件、此前的假设，以及此前程序语句的效果。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
pub fn bounded_square(x: u8) -> u8 {
    let y = x * x;
    contract_assert!(y <= 225);
    y
}
```

断言描述乘法执行后局部变量 `y` 的值。

## 实现状态

宏将条件展开为 `Fn() -> bool + Copy` 闭包。前端将其提取为程序相应位置的断言。已有集成样例和 LLBC 快照覆盖断言。

`contract_assert!` 是规约标注，在普通构建中会被移除，不提供 Rust `assert!` 宏的运行时检查。
