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

基本参数条件已支持前端提取。后端已有的用例包括用 `x < 16` 约束平方计算，以及用 `a >= b` 约束无符号减法。

## 实现边界

- **前端 `rust-llbc`**：已支持基本参数条件、方法接收者和多条前置条件。
- **后端 `llbc-why3`**：已支持基本标量条件的翻译和验证。

无函数体声明的限制见[函数契约](function-contracts.md#contract.function.boundary-title)；切片访问等条件的限制见[逻辑表达式](expressions.md#contract.expression.boundary-title)。
