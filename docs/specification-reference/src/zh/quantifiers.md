{{#rule contract.quantifier}}

# 量词

{{#syntax Quantifier QuantifierPredicate}}

{{#rule contract.quantifier.intro}}

## 语义

`forall!(|x: T| P)` 表示对类型 `T` 的每个值 `x`，`P` 都成立。`exists!(|x: T| P)` 表示存在类型 `T` 的某个值 `x`，使 `P` 成立。两种形式中的 `P` 都必须具有 `bool` 类型。

`x` 的绑定作用域是闭包体。闭包体可以引用外部绑定，也可以包含其他量词。多个量化变量通过嵌套量词表达，每个量词接受一个参数。

量化域是 Rust 类型。例如，对 `u8` 的量化涉及其有限取值范围，而非无界数学整数。对量化域的限制写在量词体中：全称量化使用条件性质，存在量化则要求范围限制和性质同时成立。

{{#rule contract.quantifier.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| forall!(|i: u8| i <= result))]
pub fn largest_u8() -> u8 {
    u8::MAX
}

#[postcondition(|result| exists!(|i: u8| i == result))]
pub fn zero() -> u8 {
    0
}
```

第一个后置条件给出所有 `u8` 值的上界。第二个后置条件声明，在同一类型中存在一个值与返回值相等。

{{#rule contract.quantifier.boundary}}

## 实现状态 <a id="contract.quantifier.boundary-title"></a>

闭包必须满足 `Fn(T) -> bool + Copy`，并且只接受一个量化值。`|x: T, y: U|` 不是该接口的双变量形式。前端递归提取规约闭包，包括嵌套量词。

文法使用显式参数类型。省略类型依赖 Rust 的类型推断。当前没有专用的区间绑定、集合绑定或独立的数学类型量化域。

`forall!` 和 `exists!` 自身不改写 `implies` 或 `old`。改写由外层契约宏完成，且只有后置条件处理 `old`。

两种量词均有宏和提取支持。已有集成样例与快照覆盖包含 `forall` 的组合用法，但尚未提供同等的 `exists` 覆盖。
