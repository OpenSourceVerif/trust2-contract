{{#rule contract.statement}}

# 语句标注

语句标注描述函数体内某个位置的条件。

{{#syntax ContractStatement}}

{{#rule contract.statement.intro}}

## 语义

[断言](assertions.md)声明需要在当前位置建立的条件。[假设](assumptions.md)提供从当前位置继续推理时采用的条件。二者都可以引用作用域内的变量，并描述标注处的状态。

{{#rule contract.statement.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

pub fn check(x: u8) {
    contract_assume!(x < 16);
    contract_assert!(x <= 15);
}
```

断言可以由前面的假设推出。这不证明每次调用 `check` 时都有 `x < 16`。

{{#rule contract.statement.boundary}}

## 实现边界 <a id="contract.statement.boundary-title"></a>

- **前端 `rust-llbc`**：已支持断言和假设，保留条件、种类及所在位置。
- **后端 `llbc-why3`**：分别翻译为 WhyML `assert` 和 `assume`。

具体用法与限制见[断言](assertions.md)和[假设](assumptions.md)。
