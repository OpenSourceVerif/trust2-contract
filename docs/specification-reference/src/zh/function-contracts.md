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

{{#rule contract.function.boundary}}

## 实现状态 <a id="contract.function.boundary-title"></a>

带函数体的普通函数、嵌套函数、固有方法、关联函数、trait 默认方法和 impl 中的方法支持契约展开与提取。已有集成样例覆盖前置条件和后置条件。

仅有签名的 trait 方法和外部函数声明尚不支持。当前没有通用的 trait 契约继承或精化机制。验证模式下的宏展开不支持 `const fn`；前端尚不能提取异步函数。

当前契约接口没有修改集合、终止度量或异常退出子句。各类条件的类型和提取限制在后续页面说明。
