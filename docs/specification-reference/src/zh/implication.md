{{#rule contract.implies}}

# 蕴含

{{#syntax Implication}}

{{#rule contract.implies.intro}}

## 语义

`P ==> Q` 表示逻辑蕴含：仅在 `P` 为真且 `Q` 为假时为假。`P` 和 `Q` 都具有 `bool` 类型。

`==>` 的优先级低于其他所有运算符，并且右结合：`a && b ==> c == d` 表示 `(a && b) ==> (c == d)`，`a ==> b ==> c` 表示 `a ==> (b ==> c)`。要把结果作为操作数使用，需要加括号，例如 `(a ==> b) && c`。写在 `if` 或 `match` 之后的条件同样要加括号：`if (a ==> b) { ... }`。

规约宏把 `P ==> Q` 改写为 `!(P) || (Q)`，因此只有 `P` 成立时才求值 `Q`。这样，关于下标的条件可以保护依赖它的访问，例如 `i < v.len() ==> v[i] > 0`。

`implies(P, Q)` 的真值表相同，但它是普通调用，两个参数都会求值。建议使用 `==>`。

{{#rule contract.implies.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| x > 0 ==> result >= x)]
pub fn identity(x: i32) -> i32 {
    x
}

#[precondition(forall!(|i: usize| i < v.len() ==> v[i] > 0))]
pub fn all_positive(v: &[u8]) {}
```

第一个后置条件要求：只要 `x > 0`，就有 `result >= x`。第二个条件不会读取切片范围之外的 `v[i]`。

{{#rule contract.implies.boundary}}

## 实现边界 <a id="contract.implies.boundary-title"></a>

- **前端 `rust-llbc`**：`==>` 由规约宏改写，在 LLBC 中表现为分支，没有专门的标记。`implies` 被识别并作为标记保留在 LLBC 中。
- **后端 `llbc-why3`**：`==>` 翻译为 WhyML 条件表达式，例如 `if x > 0 then result >= x else true`。`implies` 标记尚未翻译。涉及切片的条件受[后端切片支持](expressions.md#contract.expression.boundary-title)限制。

### 尚待补齐

**`implies` 标记（后端）。** `implies(x > 0, result >= x)` 需要翻译；也可以移除 `implies`，统一使用 `==>`。
