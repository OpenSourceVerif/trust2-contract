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

前端可以提取这类局部断言，也支持在条件中保留普通函数调用，例如 `contract_assert!(decuple(9) == 90)`。

## 实现边界

- **前端 `rust-llbc`**：已支持断言条件的提取，保留其在函数体中的位置，包括分支内的断言。
- **后端 `llbc-why3`**：已翻译为 WhyML `assert`；条件能否继续验证取决于其中的表达式和程序操作。

共同限制见[逻辑表达式](expressions.md#contract.expression.boundary-title)。普通构建移除契约断言；运行时检查使用 Rust 的 `assert!`。
