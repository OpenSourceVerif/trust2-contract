{{#rule contract.quantifier}}

# 量词

{{#syntax Quantifier QuantifierPredicate}}

{{#rule contract.quantifier.intro}}

## 语义

`forall!(|x: T| P)` 表示对类型 `T` 的每个值 `x`，`P` 都成立。`exists!(|x: T| P)` 表示存在类型 `T` 的某个值 `x`，使 `P` 成立。两种形式中的 `P` 都必须具有 `bool` 类型。

`x` 的绑定作用域是闭包体。闭包体可以引用外部绑定，也可以包含其他量词。多个量化变量可以通过嵌套量词表达，也可以将一个元组作为参数并解构。每个量词接受一个参数，`|x: u8, y: u8|` 这样的双参数闭包不符合当前接口。

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

这两个例子中的量词及返回值绑定都能进入 LLBC，并生成对应的 WhyML 量化条件。

元组参数可以同时引入两个变量。例如，下面的条件表示任取两个 `u8` 值，它们相等或返回值为零：

```rust,ignore
#[postcondition(|r| forall!(|(x, y): (u8, u8)| x == y || r == 0))]
pub fn zero_for_pairs() -> u8 {
    0
}
```

这一写法也能提取并生成 WhyML。涉及其他外层变量的捕获和嵌套组合，见下面的实现边界。

{{#rule contract.quantifier.boundary}}

## 实现边界 <a id="contract.quantifier.boundary-title"></a>

- **前端 `rust-llbc`**：已有两种量词及嵌套量词的提取，元组参数也能保留；外层变量的捕获传递仍有问题。
- **后端 `llbc-why3`**：已有全称与存在量词翻译，但生成文件不代表绑定和条件均已正确处理。

### 尚待补齐

**外层变量的传递（前端提取与后端使用）。** 下面的条件表示返回值与输入相等，但目前尚不能正确翻译：

```rust,ignore
#[postcondition(|r| exists!(|x: u8| x == r && x == n))]
pub fn identity(n: u8) -> u8 {
    n
}
```

该例能生成 LLBC，后续 WhyML 却引用了未定义变量。量词引用函数参数时，以及多层量词同时引用外层变量时，都存在这类问题。需要在提取和翻译之间正确传递各层捕获，使 `n` 始终指向输入、`r` 指向返回值、`x` 指向本层绑定。

区间、切片上的量化还受[相应表达式](expressions.md#contract.expression.boundary-title)的支持限制。例如“所有有效下标对应的元素都为正”，需要量词、长度和索引操作一起工作。
