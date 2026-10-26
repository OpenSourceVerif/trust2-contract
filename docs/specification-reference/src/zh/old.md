{{#rule contract.old}}

# 旧状态表达式

{{#syntax OldExpression}}

{{#rule contract.old.intro}}

## 语义

后置条件中的旧状态表达式引用函数的前状态，即进入函数时的状态，用于描述返回时状态与先前状态之间的关系。

`old(p)` 接受函数的一个可变引用参数 `p`，表示该引用在入口处的状态：`*old(p)` 是入口时 `p` 指向的值，`*p` 是返回时的值。`old` 是作用于参数的标记，而不是对任意 Rust 表达式的快照操作：不接受 `old(*p)`、`old(x + 1)` 或 `old(v.len())`。只有 `&mut` 参数的入口值会与返回值不同；其他参数本来就表示入口值。

{{#rule contract.old.example}}

## 示例

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(*p < u32::MAX)]
#[postcondition(|_| *p == *old(p) + 1)]
pub fn increment(p: &mut u32) {
    *p += 1;
}

#[postcondition(|_| forall!(|i: usize| implies(i < v.len(), v[i] == old(v)[i] / 2)))]
pub fn halve_all(v: &mut [u32]) {
    for x in v.iter_mut() {
        *x /= 2;
    }
}
```

`old` 可以出现在后置条件内嵌套的量词中，也适用于切片等动态大小的被指向类型。后置条件宏把 `old(p)` 改写为 `old(&p)`，使闭包以共享引用捕获 `p`，从而保持 `Fn + Copy`；参数不是 `&mut` 时产生类型错误。

{{#rule contract.old.boundary}}

## 实现边界 <a id="contract.old.boundary-title"></a>

- **前端 `rust-llbc`**：把 `old(p)` 提取为 `SpecOld` 调用，其参数是后置条件对 `p` 的捕获。在后置条件以外使用 `old`，或参数不是函数的参数（例如返回的引用、量词变量或字段 `old(s.f)`）时报错。
- **后端 `llbc-why3`**：尚未实现旧状态翻译；`SpecOld` 和可变借用还不能翻译。

### 尚待补齐

**可变参数的入口值（后端）。** 对上面的 `increment`，后端需要利用可变引用的二元组表示，把 `*old(p)` 翻译为 `*p` 的入口值，把 `*p` 翻译为返回时的值。

**字段与复合值（前后端）。** 对 `s: &mut S` 及其字段 `f: &mut T`，`*s.f` 的入口值如何表示，以及经由多个引用可达的数据如何保留旧状态，范围仍待明确。
