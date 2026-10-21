{{#rule contract.postcondition}}

# 后置条件

{{#syntax Postcondition ResultPredicate}}

## 语义

`#[postcondition(|result| Q)]` 要求布尔条件 `Q` 在正常返回时成立，其中 `result` 绑定返回值。验证函数时，必须在前置条件成立的前提下建立这一条件。

多个后置条件声明同时成立的保证。它们不规定函数是否终止，也不规定发生 panic 时的行为。

## 返回值绑定 <a id="result-binding"></a>

参数名可以任意选取，`result` 不是关键字。其类型是函数的返回类型，也可以显式写出。未声明返回类型的函数返回 `()`。

函数参数和方法接收者通过外部作用域引用。捕获一个引用，本身不表示对其指向的数据创建快照。前状态记法的含义及其实现限制见[旧状态表达式](old.md)。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|value: u32| value == x)]
pub fn identity(x: u32) -> u32 {
    x
}
```

这里 `value` 表示返回值，`x` 是参与比较的输入。

## 实现状态

后置条件必须满足 `Fn(ReturnType) -> bool + Copy`。宏将函数返回类型传入内部接口，由 Rust 检查闭包参数、结果类型和捕获。

前端提取后置条件并将其关联到函数，已有集成样例覆盖该路径。文法展示内联闭包；能够解析某个表达式，不代表支持任意函数指针或可调用对象。

只有后置条件启用 `old` 的名称改写。这不提供通用快照，也不放宽闭包的借用约束。
