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

{{#rule contract.invariant.boundary}}

## 实现状态 <a id="contract.invariant.boundary-title"></a>

宏保留泛型参数和约束，生成包含 `invariant(&self) -> bool` 方法的 `TypeInvariant` 实现。前端提取条件并将其关联到类型。已有集成快照覆盖泛型 `RefRange` 样例。

实现尚不会在构造、字段更新或借用结束处自动插入建立或恢复不变式的义务。仅声明这一属性，不能保证所有实例都满足条件。

类型别名不能使用该属性。多个要求应写在一个条件中，例如通过 `&&` 组合；重复属性会生成冲突的 trait 实现。宏分支接受枚举和联合体，但所列集成样例尚未覆盖这两类用法。联合体字段访问仍受 Rust 安全规则约束。
