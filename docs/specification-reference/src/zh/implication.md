{{#rule contract.implies}}

# 蕴含

{{#syntax Implication}}

{{#rule contract.implies.intro}}

## 语义

`implies(P, Q)` 表示逻辑蕴含，两个参数都具有 `bool` 类型。该命题仅在 `P` 为真且 `Q` 为假时为假，其逻辑含义是 `!P || Q`。

这一等价关系针对真值。当前的调用表示不会赋予 `implies` 短路运算符的求值行为。

{{#rule contract.implies.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| implies(x > 0, result >= x))]
pub fn identity(x: i32) -> i32 {
    x
}
```

后置条件要求：只要 `x > 0`，就有 `result >= x`。

{{#rule contract.implies.boundary}}

## 实现状态 <a id="contract.implies.boundary-title"></a>

契约宏将 `implies` 改写为内部函数，前端识别并保留 `SpecImplies` 标记。已有组合样例覆盖这一表示。

两个参数的计算都会进入规约体。特别是，`implies(i < a.len(), a[i] > 0)` 不会自动用前件保护索引访问。

访问需要有条件地进行时，应使用 Rust 短路运算符。例如，`i >= a.len() || a[i] > 0` 在 `i` 越界时不会求值索引表达式。
