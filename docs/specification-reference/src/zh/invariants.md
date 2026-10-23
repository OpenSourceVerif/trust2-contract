{{#rule contract.invariant}}

# 类型不变式

{{#syntax TypeInvariant}}

{{#rule contract.invariant.intro}}

## 语义

`#[invariant(P)]` 为结构体、枚举或联合体关联一个布尔条件 `P`。条件中的 `self` 指代被描述的值。该条件表达有效实例预期满足的性质。

声明不变式与在程序边界强制保持不变式是两项不同的能力。当前实现提供声明和提取，尚未实现自动保持检查。

{{#rule contract.invariant.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[invariant(self.start <= self.end)]
pub struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}
```

条件要求范围起点不大于终点。类型参数及其约束在不变式中仍然可用。

前端可以提取这个泛型结构体的不变式，并保留它与类型的关联。

枚举可以分别描述各变体的条件：

```rust,ignore
#[invariant(match self {
    Self::Empty => true,
    Self::Range { start, end } => start <= end,
})]
pub enum Range {
    Empty,
    Range { start: u32, end: u32 },
}
```

前端可以提取这个 `match self` 条件。联合体条件也能提取；读取字段仍需遵循 Rust 的 `unsafe` 要求：

```rust,ignore
#[invariant(unsafe { self.x <= 100 })]
pub union Data {
    pub x: u32,
    pub y: u32,
}
```

这些例子说明条件如何与类型关联，尚不表示后端会自动维护这些条件。

{{#rule contract.invariant.boundary}}

## 实现边界 <a id="contract.invariant.boundary-title"></a>

- **前端 `rust-llbc`**：已支持结构体、枚举和联合体不变式的提取，保留它与类型的关联。同一类型的多个条件目前需用 `&&` 合写，重复属性会产生冲突。
- **后端 `llbc-why3`**：尚未使用这些条件建立不变式检查；声明不变式还不能保证对象有效。

### 尚待补齐

**从类型条件到检查义务（后端，检查时机待明确）。** 下面的类型要求 `x <= 100`，但创建越界实例目前不会自动产生相应检查：

```rust,ignore
#[invariant(self.x <= 100)]
pub struct B {
    pub x: u32,
}

pub fn invalid() -> B {
    B { x: 200 }
}
```

前端会保存 `x <= 100`；生成的 WhyML 尚未使用它。需要让这条条件参与对象创建、修改及对外使用时的验证。具体在哪些边界检查、是否允许修改过程中暂时不满足条件，以及借用结束时如何恢复，仍待明确。
