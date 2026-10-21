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

## 实现状态 <a id="contract.statement.boundary-title"></a>

前端分别记录断言和假设的种类及位置。条件使用 `Fn() -> bool + Copy` 闭包，遵循 Rust 的捕获和借用规则。普通构建会移除这些标注。

标注可以出现在分支或循环中。循环体内的语句标注不是循环不变式声明。当前没有循环不变式、终止度量或附着于任意语句块的契约接口。
