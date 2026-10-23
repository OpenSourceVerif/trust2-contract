{{#rule contract.function}}

# 函数契约

函数契约规定调用函数时需要满足的条件，以及函数正常返回时保证的条件。

{{#syntax FunctionContract}}

{{#rule contract.function.intro}}

## 语义

<a id="grammar-Precondition"></a>[前置条件](preconditions.md)描述调用者的义务，也是验证函数体时采用的假设。<a id="grammar-Postcondition"></a>[后置条件](postconditions.md)描述函数返回时的义务。

属性放在函数定义之前。前置条件和后置条件可以重复声明，也可以交错排列。所有已声明的前置条件都必须满足，所有已声明的后置条件都需要建立。后置条件本身不声明函数终止。

<a id="grammar-ResultPredicate"></a><a id="result-binding"></a>后置条件通过闭包参数绑定返回值，参见[返回值绑定](postconditions.md#result-binding)。对先前状态的引用使用[旧状态表达式](old.md)，其当前接口仍有限制。

{{#rule contract.function.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

前置条件将 `x` 限制在平方结果能够用 `u8` 表示的范围内。后置条件要求返回值不小于 `x`。

这个例子已支持前端提取和后端验证。前置条件和后置条件也可以分别使用，或在同一函数上声明多个。

方法上的条件可以引用 `self`。下面的契约能够提取并生成 WhyML：

```rust,ignore
pub struct Counter {
    value: u32,
}

impl Counter {
    #[precondition(self.value < 10)]
    #[postcondition(|r| r == self.value)]
    pub fn get(&self) -> u32 {
        self.value
    }
}
```

{{#rule contract.function.boundary}}

## 实现边界 <a id="contract.function.boundary-title"></a>

- **前端 `rust-llbc`**：已支持普通函数和带函数体的方法上的前置／后置条件。
- **后端 `llbc-why3`**：已支持基本标量函数的契约验证；泛型函数和 trait 声明仍有翻译限制。

### 尚待补齐

**无函数体的声明（前端）。** 当前宏通过函数体携带条件，下面的写法无法展开：

```rust,ignore
trait Reader {
    #[postcondition(|result| result <= 100)]
    fn read(&self) -> u8;
}
```

这里需要能把条件保存到方法声明上，供后续处理实现和调用时使用。契约如何由实现沿用，以及是否允许加强保证，仍待明确。

**特殊函数形式（前端）。** `#[precondition(x < 100)] const fn f(x: u32) -> u32 { x }` 的展开会调用非常量辅助函数；`#[postcondition(|r| r > 0)] async fn f() -> u32 { 1 }` 则在前端提取时遇到尚不支持的协程类型。要覆盖这些形式，需要分别适配常量上下文，以及异步函数完成后得到的返回值。
