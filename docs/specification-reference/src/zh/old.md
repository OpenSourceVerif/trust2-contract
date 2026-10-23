{{#rule contract.old}}

# 旧状态表达式

{{#syntax OldExpression}}

{{#rule contract.old.intro}}

## 预期语义

后置条件中的旧状态表达式引用函数的前状态，即进入函数时的状态，用于描述返回时状态与先前状态之间的关系。

当前 `old` 构造为此提供一个标记，其接口受下述限制。它尚未提供对 Rust 表达式的通用快照操作。

{{#rule contract.old.example}}

## 语法示例

```rust,ignore
*old(p)
```

该片段将旧状态标记与解引用组合。参数 `p` 必须符合可变引用接口。片段能否用于后置条件，还取决于外层闭包的捕获和借用。

{{#rule contract.old.boundary}}

## 实现边界 <a id="contract.old.boundary-title"></a>

- **前端 `rust-llbc`**：已识别 `old` 标记，但还不能完整表达入口值；当前接口只接受可变引用。
- **后端 `llbc-why3`**：尚未实现旧状态翻译。

### 尚待补齐

**可变参数的入口值（前后端）。** 以下例子希望描述一次加一操作，目前尚不能使用：

```rust,ignore
#[precondition(*p < u32::MAX)]
#[postcondition(|_| *p == *old(p) + 1)]
fn increment(p: &mut u32) {
    *p += 1;
}
```

这段写法在验证模式下会被闭包的 `Copy` 要求挡住。前端需要允许后置条件引用入口值，并将它与返回时的值区分；后端需要利用这一表示建立两者的关系。希望表达的是 `*p` 比入口值大 1，具体接口可以随状态表示调整。

字段、复合值以及涉及别名的数据如何保留旧状态，范围仍待明确。
