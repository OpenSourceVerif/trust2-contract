{{#rule contract.postcondition}}

# 后置条件

{{#syntax Postcondition ResultPredicate}}

## 语义

`#[postcondition(|result| Q)]` 要求布尔条件 `Q` 在正常返回时成立，其中 `result` 绑定返回值。验证函数时，必须在前置条件成立的前提下建立这一条件。

多个后置条件声明同时成立的保证。它们不规定函数是否终止，也不规定发生 panic 时的行为。

## 返回值绑定 <a id="result-binding"></a>

参数名可以任意选取，`result` 不是关键字。其类型是函数的返回类型，也可以显式写出。未声明返回类型的函数返回 `()`。

函数参数和方法接收者通过外部作用域引用。捕获一个引用，本身不表示对其指向的数据创建快照。前状态记法的含义及其实现限制见[旧状态表达式](old.md)。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|value: u32| value == x)]
pub fn identity(x: u32) -> u32 {
    x
}
```

这里 `value` 表示返回值，`x` 是参与比较的输入。

返回值条件已支持前端提取和基本后端验证。例如，最大值函数可以分别声明 `result >= a` 和 `result >= b`，同时保证结果不小于两个输入。

## 实现边界

- **前端 `rust-llbc`**：已支持闭包绑定返回值、引用输入参数和多条后置条件。
- **后端 `llbc-why3`**：已支持基本标量返回值条件；涉及可变引用和旧状态的关系仍不完整。

### 尚待补齐

**直接传入函数（前端）。** 以下写法能够通过 Rust 类型检查，但规约提取会失败：

```rust,ignore
fn positive(value: u32) -> bool {
    value > 0
}

#[postcondition(positive)]
pub fn one() -> u32 {
    1
}
```

同一个条件写成 `#[postcondition(|value| positive(value))]` 可以提取。直接传入函数时，还需要将它关联到返回值条件，或在入口给出明确的不支持提示；函数调用本身的验证限制见[逻辑表达式](expressions.md#contract.expression.boundary-title)。

**修改前后的关系。** 例如“`*p` 返回时比进入函数时大 1”，用 [`old`](old.md) 写作 `*p == *old(p) + 1`。验证它仍需要[后端支持](old.md#contract.old.boundary-title)旧状态和可变引用。
