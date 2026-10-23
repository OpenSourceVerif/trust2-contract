{{#rule contract.assumption}}

# 假设

{{#syntax Assumption}}

## 语义

`contract_assume!(P);` 在当前位置将布尔条件 `P` 引入为推理假设，不产生证明 `P` 的义务。

使用这一假设得到的结论以该假设成立为前提。不一致的假设会使后续证明义务空泛成立。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

pub fn assumed_bound(x: u8, flag: bool) {
    if flag {
        contract_assume!(x <= 10);
        contract_assert!(x < 11);
    } else {
        contract_assert!(x <= 255);
    }
}
```

在 `flag` 为真的分支中，假定的上界足以推出随后的断言；另一分支的断言不使用这条假设。前端提取和 WhyML 翻译会保留这种分支关系。该标注既不检查，也不为调用者建立这个上界。

## 实现边界

- **前端 `rust-llbc`**：已支持假设提取，与断言区分，并保留所在分支。
- **后端 `llbc-why3`**：已翻译为 WhyML `assume`；条件能否继续验证取决于其中的表达式和程序操作。

共同限制见[逻辑表达式](expressions.md#contract.expression.boundary-title)。假设不产生证明自身的义务，普通构建也不会用它过滤输入。
