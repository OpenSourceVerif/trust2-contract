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

前端能够提取这种蕴含表达式，也能在量词条件中保留它。

{{#rule contract.implies.boundary}}

## 实现边界 <a id="contract.implies.boundary-title"></a>

- **前端 `rust-llbc`**：已识别并在 LLBC 中保留 `implies` 标记，包括量词体中的用法。
- **后端 `llbc-why3`**：尚未翻译这一标记，上例还不能完成验证。

### 尚待补齐

**逻辑蕴含翻译（后端）。** 对上例的 `implies(x > 0, result >= x)`，需要生成“当 `x > 0` 时，`result >= x`”这一逻辑条件；量词体中的标记也需要按同样含义处理。

当前两个参数都会求值，`implies(i < a.len(), a[i] > 0)` 因而不能用前件保护索引访问。需要短路行为时，可以写成 `i >= a.len() || a[i] > 0`；这仍受切片与索引的后端支持限制。是否让 `implies` 本身具有条件求值行为，尚待明确。
