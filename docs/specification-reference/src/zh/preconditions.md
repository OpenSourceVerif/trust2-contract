{{#rule contract.precondition}}

# 前置条件

{{#syntax Precondition}}

## 语义

`#[precondition(P)]` 要求布尔条件 `P` 在进入函数时成立。在调用处，调用者必须建立 `P`；验证函数体时，将 `P` 作为假设。

条件可以引用函数参数、方法接收者，以及定义处可见的其他名称。前置条件不绑定返回值。多个前置条件属性要求其中的条件全部成立。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(divisor != 0)]
pub fn quotient(value: u32, divisor: u32) -> u32 {
    value / divisor
}
```

前置条件排除了除零情况。以 `divisor == 0` 调用该函数不满足所声明的要求。

## 实现状态

宏将条件解析为表达式，并表示为 `Fn() -> bool + Copy` 闭包。Rust 的类型、捕获和借用约束仍然适用。前端将提取出的条件作为前置条件关联到函数。

已有集成样例和 LLBC 快照覆盖前置条件。支持的函数形式和共同限制见[函数契约](function-contracts.md#contract.function.boundary-title)。
