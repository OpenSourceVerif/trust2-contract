{{#rule contract.expression}}

# 逻辑表达式

逻辑表达式构成契约、语句标注和类型不变式中的条件。它们使用 Rust 表达式，并扩展了量词、蕴含和旧状态标记。

{{#syntax Condition Expr Arguments Field UnaryOp BinaryOp}}

{{#rule contract.expression.intro}}

## 类型与值

`Condition` 必须具有 `bool` 类型。子表达式可以具有其他 Rust 类型：例如，在 `x + 1 <= limit` 中，加法产生数值，比较产生布尔值。当前没有独立的谓词类型，也不将整数隐式转换为布尔条件。

名称引用作用域内的绑定。字段访问、解引用、索引、函数调用和方法调用遵循 Rust 的类型与借用规则。圆括号用于分组，不改变表达式的值。

整数表达式使用 Rust 整数类型，遵循其有限取值范围和算术检查。规约接口没有引入数学整数或实数。访问集合元素的表达式同样必须满足索引边界要求；表达式出现在规约中不会免除这一要求。

## 运算符 <a id="operators"></a>

下表按优先级从高到低排列。一元运算符右结合，算术和位运算的二元运算符左结合。比较不能直接连写，应写成 `a < b && b < c`。

| 分组 | 运算符 |
| --- | --- |
| 访问与调用 | `e.field` `e[i]` `f(x)` `e.method(x)` |
| 一元运算 | `!` `-` `*` `&` `&mut` |
| 乘除与取余 | `*` `/` `%` |
| 加减 | `+` `-` |
| 移位 | `<<` `>>` |
| 按位与 | `&` |
| 按位异或 | `^` |
| 按位或 | `\|` |
| 比较 | `==` `!=` `<` `<=` `>` `>=` |
| 逻辑与 | `&&` |
| 逻辑或 | `\|\|` |

对布尔操作数，`!`、`&&` 和 `||` 分别表示否定、合取和析取。`&&` 和 `||` 短路求值，只有需要时才求值右操作数。单字符的位运算符不提供这种短路行为。

[量词](quantifiers.md)绑定某个 Rust 类型的值；[蕴含](implication.md)表达条件命题；[旧状态表达式](old.md)说明后置条件中可用的前状态标记。

{{#rule contract.expression.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(i < values.len() && values[i] > 0)]
pub fn positive_at(values: &[u8], i: usize) -> u8 {
    values[i]
}
```

该条件要求索引有效且对应元素为正。短路求值保证仅在索引合法时才求值第二个操作数。

前端已有切片长度、Vec 索引、字段比较和普通调用等表达式的提取用例；后端对这些表达式的支持仍有差异，见下文。

条件也可以使用 Rust 表达式块、局部绑定、类型转换和分支。例如，下面的前置条件能够提取：

```rust,ignore
#[precondition({
    let y = x as u32;
    if flag { y > 0 } else { y == 0 }
})]
pub fn selected(x: u8, flag: bool) -> u8 {
    x
}
```

枚举条件可以使用 `match`，见[类型不变式中的示例](invariants.md#r-contract.invariant.example)。普通函数调用也能进入规约，例如 `#[precondition(positive(x))]`，其中 `positive` 是返回 `bool` 的 Rust 函数。这些表达式能提取，不表示其所有操作都已接入后端验证。

{{#rule contract.expression.boundary}}

## 实现边界 <a id="contract.expression.boundary-title"></a>

- **前端 `rust-llbc`**：已能提取基本运算、字段访问、切片访问、普通调用，以及条件中的 `if`、`match` 和类型转换。
- **后端 `llbc-why3`**：已支持基础标量表达式；切片、索引、`usize`、部分枚举与 trait 操作仍有缺口。

### 尚待补齐

**切片条件（后端）。** 上例的 `i < values.len() && values[i] > 0` 可以提取，但验证还需要切片、长度、下标类型和索引操作的翻译，以便表达“索引有效且元素为正”。

**规约中的函数调用（前后端，规则待明确）。** `#[precondition(positive(x))]` 这类普通调用可以进入 LLBC。当前前端也会接受调用一个修改全局状态、再返回 `bool` 的函数；`Fn + Copy` 约束本身并不排除这种副作用。哪些函数可以用于规约、如何约束其副作用，需要先明确，再由前端检查和后端逻辑表示共同落实。普通调用能够提取，还不足以保证它可作为逻辑条件完成验证。
